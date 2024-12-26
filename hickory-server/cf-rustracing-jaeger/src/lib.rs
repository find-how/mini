//! [Jaeger][jaeger] client library created on top of [rustracing].
//!
//! [jaeger]: https://github.com/jaegertracing/jaeger
//! [rustracing]: https://crates.io/crates/rustracing
//!
//! # Examples
//!
//! ```
//! use cf_rustracing::sampler::AllSampler;
//! use cf_rustracing_jaeger::Tracer;
//! use cf_rustracing_jaeger::reporter::JaegerCompactReporter;
//! use std::net::Ipv4Addr;
//!
//! # #[tokio::main]
//! # async fn main() {
//! // Creates a tracer
//! let (tracer, mut span_rx) = Tracer::new(AllSampler);
//! {
//!     let span = tracer.span("sample_op").start();
//!     // Do something
//!
//! } // The dropped span will be sent to `span_rx`
//!
//! let span = span_rx.recv().await.unwrap();
//! assert_eq!(span.operation_name(), "sample_op");
//!
//! // Reports this span to the local jaeger agent
//! let reporter = JaegerCompactReporter::new(
//!     "sample_service",
//!     (Ipv4Addr::LOCALHOST, 6831).into(),
//!     (Ipv4Addr::LOCALHOST, 0).into(),
//! )
//! .await
//! .unwrap();
//!
//! reporter.report(&[span]).await.unwrap();
//! # }
//! ```

#![warn(missing_docs)]
#[macro_use]
extern crate trackable;

pub use self::span::Span;
pub use self::tracer::Tracer;
pub use cf_rustracing::{Error, ErrorKind, Result};

pub mod reporter;
pub mod span;
pub mod thrift;

mod constants;
mod error;
mod tracer;

#[cfg(test)]
mod tests {
    use crate::reporter::JaegerCompactReporter;
    use crate::Tracer;
    use cf_rustracing::sampler::AllSampler;
    use cf_rustracing::tag::Tag;
    use std::net::Ipv4Addr;

    #[tokio::test]
    async fn it_works() {
        let (tracer, mut span_rx) = Tracer::new(AllSampler);
        {
            let _span = tracer.span("it_works").start();
            // do something
        }
        let span = span_rx.recv().await.unwrap();
        assert_eq!(span.operation_name(), "it_works");

        let mut reporter = JaegerCompactReporter::new(
            "sample_service",
            (Ipv4Addr::LOCALHOST, 6831).into(),
            (Ipv4Addr::LOCALHOST, 0).into(),
        )
        .await
        .unwrap();

        reporter.add_service_tag(Tag::new("foo", "bar"));
        reporter.report(&[span]).await.unwrap();
    }
}
