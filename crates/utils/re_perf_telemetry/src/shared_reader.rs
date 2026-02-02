use std::sync::{Arc, Weak};
use std::time::Duration;

use opentelemetry_sdk::error::OTelSdkResult;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::metrics::reader::MetricReader;
use opentelemetry_sdk::metrics::{ManualReader, Pipeline, Temporality};

/// A wrapper that allows sharing a `ManualReader`.
///
/// 1. `MeterProvider::with_reader()` takes ownership of the reader
/// 2. We need the same reader instance in our metrics server to call `collect()`
/// 3. `ManualReader` doesn't implement Clone
///
/// The wrapper holds `ManualReader` in Arc and implements `MetricReader` by delegating all calls.
#[derive(Clone, Debug)]
pub struct SharedManualReader {
    inner: Arc<ManualReader>,
}

impl SharedManualReader {
    pub fn new(temporality: Temporality) -> Self {
        let reader = ManualReader::builder()
            .with_temporality(temporality)
            .build();
        Self {
            inner: Arc::new(reader),
        }
    }

    pub fn inner(&self) -> Arc<ManualReader> {
        Arc::clone(&self.inner)
    }
}

impl MetricReader for SharedManualReader {
    fn register_pipeline(&self, pipeline: Weak<Pipeline>) {
        self.inner.register_pipeline(pipeline);
    }

    fn collect(&self, rm: &mut ResourceMetrics) -> OTelSdkResult {
        self.inner.collect(rm)
    }

    fn force_flush(&self) -> OTelSdkResult {
        self.inner.force_flush()
    }

    fn shutdown(&self) -> OTelSdkResult {
        self.inner.shutdown()
    }

    fn shutdown_with_timeout(&self, timeout: Duration) -> OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }

    fn temporality(&self, kind: opentelemetry_sdk::metrics::InstrumentKind) -> Temporality {
        self.inner.temporality(kind)
    }
}
