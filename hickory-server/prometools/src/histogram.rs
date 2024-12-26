//! A faster, lock-free histogram for tracking timing data.
//!
//! This is based on the implementation for [`prometheus_client::metrics::histogram::Histogram`],
//! with several changes made to eliminate the need for locks.

use std::time::{Duration, Instant};

use prometheus_client::encoding::text::{Encode, EncodeMetric, Encoder};
use prometheus_client::metrics::exemplar::Exemplar;
use prometheus_client::metrics::{MetricType, TypedMetric};
use std::collections::HashMap;
use std::iter::once;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// A faster, lock-free histogram for tracking time.
#[derive(Debug)]
pub struct TimeHistogram {
    inner: Arc<Inner>,
}

/// Timer to measure and record the duration of an event.
///
/// This timer can be stopped and observed at most once, either automatically
/// (when it goes out of scope) or manually. Alternatively, it can be manually
/// stopped and discarded in order to not record its value.
pub struct HistogramTimer {
    histogram: TimeHistogram,
    observed: bool,
    start: Option<Instant>,
    accumulated: Duration,
}

#[derive(Debug)]
struct Inner {
    sum: AtomicU64,
    count: AtomicU64,
    buckets: Vec<(f64, AtomicU64)>,
}

impl HistogramTimer {
    /// Pauses time tracking until `unpause` is called. Any time passed between this call and
    /// calling `unpause` or `stop` is NOT counted.
    ///
    /// If the timer is already paused, then this call has no effect.
    pub fn pause(&mut self) {
        self.accumulated += self.start.map_or(Duration::ZERO, |value| {
            Instant::now().saturating_duration_since(value)
        });
        self.start = None
    }

    /// Resumes time tracking, if the timer was paused, which means time after this call is tracked
    /// again.
    ///
    /// If the timer is already un-paused or was not paused ever, then this call has no effect.
    pub fn resume(&mut self) {
        if self.start.is_none() {
            self.start = Some(Instant::now());
        }
    }

    /// Observe, record and return timer duration (in seconds).
    ///
    /// It observes and returns a floating-point number for seconds elapsed since
    /// the timer started, recording that value to the attached histogram.
    pub fn stop_and_record(self) -> Duration {
        let mut timer = self;
        timer.observe(true)
    }

    /// Observe and return timer duration (in seconds).
    ///
    /// It returns a floating-point number of seconds elapsed since the timer started,
    /// without recording to any histogram.
    pub fn stop_and_discard(self) -> Duration {
        let mut timer = self;
        timer.observe(false)
    }

    fn observe(&mut self, record: bool) -> Duration {
        let elapsed_since_start = self.start.map_or(Duration::ZERO, |value| {
            Instant::now().saturating_duration_since(value)
        });
        let elapsed = elapsed_since_start + self.accumulated;

        self.observed = true;
        if record {
            self.histogram.observe(elapsed.as_nanos() as u64);
        }

        elapsed
    }
}

impl Drop for HistogramTimer {
    fn drop(&mut self) {
        if !self.observed {
            self.observe(true);
        }
    }
}

impl Clone for TimeHistogram {
    fn clone(&self) -> Self {
        TimeHistogram {
            inner: self.inner.clone(),
        }
    }
}

impl TimeHistogram {
    pub fn new(buckets: impl Iterator<Item = f64>) -> Self {
        Self {
            inner: Arc::new(Inner {
                sum: Default::default(),
                count: Default::default(),
                buckets: buckets
                    .into_iter()
                    .chain(once(f64::MAX))
                    .map(|upper_bound| (upper_bound, AtomicU64::new(0)))
                    .collect(),
            }),
        }
    }

    pub fn start_timer(&self) -> HistogramTimer {
        HistogramTimer {
            histogram: self.clone(),
            observed: false,
            start: Some(Instant::now()),
            accumulated: Duration::new(0, 0),
        }
    }

    pub fn observe(&self, nanos: u64) {
        self.observe_and_bucket(nanos);
    }

    fn observe_and_bucket(&self, v: u64) -> Option<usize> {
        self.inner.sum.fetch_add(v, Ordering::Relaxed);
        self.inner.count.fetch_add(1, Ordering::Relaxed);

        let first_bucket = self
            .inner
            .buckets
            .iter()
            .enumerate()
            .find(|(_i, (upper_bound, _value))| upper_bound >= &(v as f64 * 1E-9));

        match first_bucket {
            Some((i, (_upper_bound, value))) => {
                value.fetch_add(1, Ordering::Relaxed);
                Some(i)
            }
            None => None,
        }
    }

    pub fn snapshot(&self) -> HistogramSnapshot {
        let sum = seconds(self.inner.sum.load(Ordering::Relaxed));
        let count = self.inner.count.load(Ordering::Relaxed);
        let buckets = self
            .inner
            .buckets
            .iter()
            .map(|(k, v)| (*k, v.load(Ordering::Relaxed)))
            .collect();

        HistogramSnapshot {
            sum,
            count,
            buckets,
        }
    }
}

impl TypedMetric for TimeHistogram {
    const TYPE: MetricType = MetricType::Histogram;
}

pub struct HistogramSnapshot {
    sum: f64,
    count: u64,
    buckets: Vec<(f64, u64)>,
}

impl HistogramSnapshot {
    pub fn sum(&self) -> f64 {
        self.sum
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn buckets(&self) -> &[(f64, u64)] {
        &self.buckets
    }

    fn encode_with_maybe_exemplars<S>(
        &self,
        exemplars: Option<&HashMap<usize, Exemplar<S, f64>>>,
        mut encoder: Encoder,
    ) -> Result<(), std::io::Error>
    where
        S: Encode,
    {
        encoder
            .encode_suffix("sum")?
            .no_bucket()?
            .encode_value(self.sum)?
            .no_exemplar()?;
        encoder
            .encode_suffix("count")?
            .no_bucket()?
            .encode_value(self.count)?
            .no_exemplar()?;

        let mut cummulative = 0;
        for (i, (upper_bound, count)) in self.buckets.iter().enumerate() {
            cummulative += count;
            let mut bucket_encoder = encoder.encode_suffix("bucket")?;
            let mut value_encoder = bucket_encoder.encode_bucket(*upper_bound)?;
            let mut exemplar_encoder = value_encoder.encode_value(cummulative)?;

            match exemplars.and_then(|es| es.get(&i)) {
                Some(exemplar) => exemplar_encoder.encode_exemplar(exemplar)?,
                None => exemplar_encoder.no_exemplar()?,
            }
        }

        Ok(())
    }
}

#[inline(always)]
fn seconds(val: u64) -> f64 {
    (val as f64) * 1E-9
}

impl EncodeMetric for TimeHistogram {
    fn encode(&self, encoder: Encoder) -> Result<(), std::io::Error> {
        // TODO: Would be better to use never type instead of `()`.
        self.snapshot()
            .encode_with_maybe_exemplars::<()>(None, encoder)
    }

    fn metric_type(&self) -> MetricType {
        Self::TYPE
    }
}
