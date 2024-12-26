//! Metric types that don't follow the OpenTelemetry standard exactly.

use prometheus_client::{
    encoding::text::{Encode, EncodeMetric, Encoder},
    metrics::{
        counter::{Atomic, Counter},
        MetricType, TypedMetric,
    },
};
use std::{
    io,
    ops::{Deref, DerefMut},
    sync::atomic::AtomicU64,
};

/// A wrapper of [`prometheus_client::metrics::counter::Counter`] which does
/// not suffix the name with `_total`.
#[repr(transparent)]
pub struct NonstandardUnsuffixedCounter<N = u64, A = AtomicU64>(pub Counter<N, A>);

impl<N, A> Clone for NonstandardUnsuffixedCounter<N, A> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<N, A: Default> Default for NonstandardUnsuffixedCounter<N, A> {
    fn default() -> Self {
        Self(Counter::default())
    }
}

impl<N, A> Deref for NonstandardUnsuffixedCounter<N, A> {
    type Target = Counter<N, A>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<N, A> DerefMut for NonstandardUnsuffixedCounter<N, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<N, A> TypedMetric for NonstandardUnsuffixedCounter<N, A> {
    const TYPE: MetricType = MetricType::Counter;
}

impl<N, A> EncodeMetric for NonstandardUnsuffixedCounter<N, A>
where
    N: Encode,
    A: Atomic<N>,
{
    fn encode(&self, mut encoder: Encoder) -> Result<(), io::Error> {
        let mut bucket_encoder = encoder.no_suffix()?;
        let mut value_encoder = bucket_encoder.no_bucket()?;
        let mut exemplar_encoder = value_encoder.encode_value(self.get())?;

        exemplar_encoder.no_exemplar()
    }

    fn metric_type(&self) -> MetricType {
        Self::TYPE
    }
}

/// An info gauge, similar to [`prometheus_client::metrics::info::Info`],
/// but collected as a GAUGE with no suffix.
///
/// Useful in legacy systems who don't actually use the INFO metric type.
///
/// [`Info`]: `prometheus_client::metrics::info::Info`
#[derive(Debug)]
pub struct InfoGauge<S>(S);

impl<S> InfoGauge<S>
where
    S: Encode,
{
    pub fn new(label_set: S) -> Self {
        Self(label_set)
    }
}

impl<S> TypedMetric for InfoGauge<S> {
    const TYPE: MetricType = MetricType::Gauge;
}

impl<S> EncodeMetric for InfoGauge<S>
where
    S: Encode,
{
    fn encode(&self, mut encoder: Encoder) -> Result<(), std::io::Error> {
        encoder
            .with_label_set(&self.0)
            .no_suffix()?
            .no_bucket()?
            .encode_value(1u32)?
            .no_exemplar()?;

        Ok(())
    }

    fn metric_type(&self) -> MetricType {
        Self::TYPE
    }
}
