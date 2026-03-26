//! Prometheus-specific metric conversion and encoding utilities

#![expect(clippy::cast_possible_wrap)] // u64 -> i64

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use parking_lot::{Mutex, RwLock};
use prometheus_client::encoding::{EncodeLabelSet, EncodeMetric, MetricEncoder, NoLabelSet};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::{Family, MetricConstructor};
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::{MetricType, TypedMetric};
use prometheus_client::registry::Registry;

/// Dynamic labels for metrics that support arbitrary key-value pairs
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct DynamicLabels(Vec<(String, String)>);

impl EncodeLabelSet for DynamicLabels {
    fn encode(
        &self,
        encoder: &mut prometheus_client::encoding::LabelSetEncoder<'_>,
    ) -> Result<(), std::fmt::Error> {
        let Self(labels) = self;
        for (key, value) in labels {
            let mut label_encoder = encoder.encode_label();
            let mut key_encoder = label_encoder.encode_label_key()?;
            key_encoder.write_str(key)?;
            let mut value_encoder = key_encoder.encode_label_value()?;
            value_encoder.write_str(value)?;
            value_encoder.finish()?;
        }
        Ok(())
    }
}

/// Container for different metric types with dynamic labels
pub struct MetricContainer {
    pub counters: HashMap<String, Family<DynamicLabels, Counter>>,
    pub gauges: HashMap<String, Family<DynamicLabels, Gauge<i64>>>,
    pub histograms: HashMap<
        String,
        Family<DynamicLabels, PreAggregatedHistogram, PreAggregatedHistogramConstructor>,
    >,
}

impl MetricContainer {
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
            gauges: HashMap::new(),
            histograms: HashMap::new(),
        }
    }
}

/// Encode a Prometheus registry to text format
pub fn encode_registry(registry: &Registry) -> Result<String, std::fmt::Error> {
    let mut buffer = String::new();
    prometheus_client::encoding::text::encode(&mut buffer, registry)?;
    Ok(buffer)
}

/// Convert `OpenTelemetry` `ResourceMetrics` to Prometheus metrics and return a Registry
pub fn convert_to_prometheus(
    resource_metrics: &ResourceMetrics,
    metrics: &Arc<Mutex<MetricContainer>>,
) -> Registry {
    let mut registry = Registry::default();
    // Process each scope's metrics
    for scope in resource_metrics.scope_metrics() {
        for metric in scope.metrics() {
            let metric_name = sanitize_metric_name(metric.name());

            // Handle different metric types using the enum pattern
            use opentelemetry_sdk::metrics::data::{AggregatedMetrics, MetricData};

            match metric.data() {
                AggregatedMetrics::F64(MetricData::Gauge(gauge)) => {
                    register_gauge_f64(
                        &mut registry,
                        &metric_name,
                        metric.description(),
                        gauge,
                        metrics,
                    );
                }
                AggregatedMetrics::I64(MetricData::Gauge(gauge)) => {
                    register_gauge_i64(
                        &mut registry,
                        &metric_name,
                        metric.description(),
                        gauge,
                        metrics,
                    );
                }
                AggregatedMetrics::U64(MetricData::Gauge(gauge)) => {
                    register_gauge_u64(
                        &mut registry,
                        &metric_name,
                        metric.description(),
                        gauge,
                        metrics,
                    );
                }
                AggregatedMetrics::F64(MetricData::Sum(sum)) => {
                    if sum.is_monotonic() {
                        register_counter_f64(
                            &mut registry,
                            &metric_name,
                            metric.description(),
                            sum,
                            metrics,
                        );
                    } else {
                        register_gauge_from_sum_f64(
                            &mut registry,
                            &metric_name,
                            metric.description(),
                            sum,
                            metrics,
                        );
                    }
                }
                AggregatedMetrics::I64(MetricData::Sum(sum)) => {
                    if sum.is_monotonic() {
                        register_counter_i64(
                            &mut registry,
                            &metric_name,
                            metric.description(),
                            sum,
                            metrics,
                        );
                    } else {
                        register_gauge_from_sum_i64(
                            &mut registry,
                            &metric_name,
                            metric.description(),
                            sum,
                            metrics,
                        );
                    }
                }
                AggregatedMetrics::U64(MetricData::Sum(sum)) => {
                    if sum.is_monotonic() {
                        register_counter_u64(
                            &mut registry,
                            &metric_name,
                            metric.description(),
                            sum,
                            metrics,
                        );
                    } else {
                        register_gauge_from_sum_u64(
                            &mut registry,
                            &metric_name,
                            metric.description(),
                            sum,
                            metrics,
                        );
                    }
                }
                AggregatedMetrics::F64(MetricData::ExponentialHistogram(histogram)) => {
                    register_exponential_histogram_f64(
                        &mut registry,
                        &metric_name,
                        metric.description(),
                        histogram,
                        metrics,
                    );
                }
                AggregatedMetrics::I64(MetricData::ExponentialHistogram(histogram)) => {
                    register_exponential_histogram_i64(
                        &mut registry,
                        &metric_name,
                        metric.description(),
                        histogram,
                        metrics,
                    );
                }
                AggregatedMetrics::U64(MetricData::ExponentialHistogram(histogram)) => {
                    register_exponential_histogram_u64(
                        &mut registry,
                        &metric_name,
                        metric.description(),
                        histogram,
                        metrics,
                    );
                }
                _ => {
                    // Other metric types not supported
                }
            }
        }
    }

    registry
}

// Helper functions to register different metric types

fn register_gauge_f64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    gauge: &opentelemetry_sdk::metrics::data::Gauge<f64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = gauge.data_points().collect();
    if points.is_empty() {
        return;
    }

    let gauge_family = Family::<DynamicLabels, Gauge<i64>>::default();

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        // Convert f64 to i64 with microsecond precision
        gauge_family
            .get_or_create(&labels)
            .set((point.value() * 1000000.0) as i64);
    }

    let mut container = metrics.lock();
    container
        .gauges
        .insert(name.to_owned(), gauge_family.clone());
    registry.register(name, description, gauge_family);
}

fn register_gauge_i64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    gauge: &opentelemetry_sdk::metrics::data::Gauge<i64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = gauge.data_points().collect();
    if points.is_empty() {
        return;
    }

    let gauge_family = Family::<DynamicLabels, Gauge<i64>>::default();

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        gauge_family.get_or_create(&labels).set(point.value());
    }

    let mut container = metrics.lock();
    container
        .gauges
        .insert(name.to_owned(), gauge_family.clone());
    registry.register(name, description, gauge_family);
}

fn register_gauge_u64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    gauge: &opentelemetry_sdk::metrics::data::Gauge<u64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = gauge.data_points().collect();
    if points.is_empty() {
        return;
    }

    let gauge_family = Family::<DynamicLabels, Gauge<i64>>::default();

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        gauge_family
            .get_or_create(&labels)
            .set(point.value() as i64);
    }

    let mut container = metrics.lock();
    container
        .gauges
        .insert(name.to_owned(), gauge_family.clone());
    registry.register(name, description, gauge_family);
}

fn register_counter_f64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    sum: &opentelemetry_sdk::metrics::data::Sum<f64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = sum.data_points().collect();
    if points.is_empty() {
        return;
    }

    let counter_family = Family::<DynamicLabels, Counter>::default();

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        // For counters from OTLP, we get absolute values
        // We need to increment by the value to match the current state
        counter_family
            .get_or_create(&labels)
            .inc_by(point.value() as u64);
    }

    let mut container = metrics.lock();
    container
        .counters
        .insert(name.to_owned(), counter_family.clone());
    registry.register(name, description, counter_family);
}

fn register_counter_i64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    sum: &opentelemetry_sdk::metrics::data::Sum<i64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = sum.data_points().collect();
    if points.is_empty() {
        return;
    }

    let counter_family = Family::<DynamicLabels, Counter>::default();

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        if point.value() >= 0 {
            counter_family
                .get_or_create(&labels)
                .inc_by(point.value() as u64);
        }
    }

    let mut container = metrics.lock();
    container
        .counters
        .insert(name.to_owned(), counter_family.clone());
    registry.register(name, description, counter_family);
}

fn register_counter_u64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    sum: &opentelemetry_sdk::metrics::data::Sum<u64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = sum.data_points().collect();
    if points.is_empty() {
        return;
    }

    let counter_family = Family::<DynamicLabels, Counter>::default();

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        counter_family.get_or_create(&labels).inc_by(point.value());
    }

    let mut container = metrics.lock();
    container
        .counters
        .insert(name.to_owned(), counter_family.clone());
    registry.register(name, description, counter_family);
}

fn register_gauge_from_sum_f64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    sum: &opentelemetry_sdk::metrics::data::Sum<f64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = sum.data_points().collect();
    if points.is_empty() {
        return;
    }

    let gauge_family = Family::<DynamicLabels, Gauge<i64>>::default();

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        // Convert f64 to i64 with microsecond precision
        gauge_family
            .get_or_create(&labels)
            .set((point.value() * 1000000.0) as i64);
    }

    let mut container = metrics.lock();
    container
        .gauges
        .insert(name.to_owned(), gauge_family.clone());
    registry.register(name, description, gauge_family);
}

fn register_gauge_from_sum_i64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    sum: &opentelemetry_sdk::metrics::data::Sum<i64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = sum.data_points().collect();
    if points.is_empty() {
        return;
    }

    let gauge_family = Family::<DynamicLabels, Gauge<i64>>::default();

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        gauge_family.get_or_create(&labels).set(point.value());
    }

    let mut container = metrics.lock();
    container
        .gauges
        .insert(name.to_owned(), gauge_family.clone());
    registry.register(name, description, gauge_family);
}

fn register_gauge_from_sum_u64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    sum: &opentelemetry_sdk::metrics::data::Sum<u64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = sum.data_points().collect();
    if points.is_empty() {
        return;
    }

    let gauge_family = Family::<DynamicLabels, Gauge<i64>>::default();

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        gauge_family
            .get_or_create(&labels)
            .set(point.value() as i64);
    }

    let mut container = metrics.lock();
    container
        .gauges
        .insert(name.to_owned(), gauge_family.clone());
    registry.register(name, description, gauge_family);
}

/// A histogram that holds pre-aggregated data (sum, count, buckets) directly,
/// avoiding the lossy `observe()` approximation. Implements `EncodeMetric` so
/// Prometheus text encoding emits exact values from the `OTel` source.
#[derive(Debug, Clone)]
pub struct PreAggregatedHistogram {
    inner: Arc<RwLock<PreAggregatedHistogramInner>>,
}

#[derive(Debug)]
struct PreAggregatedHistogramInner {
    sum: f64,
    count: u64,

    /// `(upper_bound, count)` pairs — non-cumulative per-bucket counts.
    /// The prometheus-client encoder converts these to cumulative during
    /// text encoding.
    buckets: Vec<(f64, u64)>,
}

impl PreAggregatedHistogram {
    /// Populate from an `OTel` exponential histogram data point.
    ///
    /// Only positive and zero observations are mapped to Prometheus buckets.
    /// Negative observations cannot be faithfully represented in Prometheus's
    /// `le`-based histogram model, so we log a warning if any are present.
    /// In practice all our histograms track durations and sizes which are
    /// always non-negative.
    fn set_from_exponential(
        &self,
        scale: i8,
        positive_bucket: &opentelemetry_sdk::metrics::data::ExponentialBucket,
        negative_bucket: &opentelemetry_sdk::metrics::data::ExponentialBucket,
        zero_count: u64,
        sum: f64,
        count: u64,
    ) {
        let negative_count: u64 = negative_bucket.counts().sum();
        if negative_count > 0 {
            tracing::warn!(
                negative_count,
                "Histogram has negative observations which \
                 cannot be represented in Prometheus and will be dropped"
            );
        }

        let positive_counts: Vec<u64> = positive_bucket.counts().collect();
        self.set_from_raw_buckets(
            scale,
            positive_bucket.offset(),
            &positive_counts,
            zero_count,
            sum,
            count,
        );
    }

    /// Populate from raw exponential histogram bucket data.
    ///
    /// `scale` controls bucket resolution: base = 2^(2^(-scale)).
    /// `offset` is the bucket index of the first entry in `positive_counts`.
    /// `positive_counts[i]` is the count for values in (base^(offset+i), base^(offset+i+1)].
    fn set_from_raw_buckets(
        &self,
        scale: i8,
        offset: i32,
        positive_counts: &[u64],
        zero_count: u64,
        sum: f64,
        count: u64,
    ) {
        let base = (2.0_f64).powf((2.0_f64).powi(-(scale as i32)));

        let mut buckets: Vec<(f64, u64)> = positive_counts
            .iter()
            .enumerate()
            .map(|(i, &c)| {
                let upper = base.powi(offset + i as i32 + 1);
                (upper, c)
            })
            .collect();

        // Place zero-count observations into the first bucket (or create one
        // before +Inf if there are no positive buckets).
        if zero_count > 0 {
            if let Some(first) = buckets.first_mut() {
                first.1 += zero_count;
            } else {
                buckets.push((0.0, zero_count));
            }
        }

        // Always add +Inf bucket — required by Prometheus exposition format.
        // Count of 0 is correct here because the encoder accumulates cumulatively.
        buckets.push((f64::MAX, 0));

        let mut inner = self.inner.write();
        inner.sum = sum;
        inner.count = count;
        inner.buckets = buckets;
    }
}

impl Default for PreAggregatedHistogram {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(PreAggregatedHistogramInner {
                sum: 0.0,
                count: 0,
                buckets: Vec::new(),
            })),
        }
    }
}

impl TypedMetric for PreAggregatedHistogram {
    const TYPE: MetricType = MetricType::Histogram;
}

impl EncodeMetric for PreAggregatedHistogram {
    fn encode(&self, mut encoder: MetricEncoder<'_>) -> Result<(), std::fmt::Error> {
        let inner = self.inner.read();
        encoder.encode_histogram::<NoLabelSet>(inner.sum, inner.count, &inner.buckets, None)
    }

    fn metric_type(&self) -> MetricType {
        Self::TYPE
    }
}

/// Default constructor for `Family<_, PreAggregatedHistogram, _>`.
#[derive(Clone, Default)]
pub struct PreAggregatedHistogramConstructor;

impl MetricConstructor<PreAggregatedHistogram> for PreAggregatedHistogramConstructor {
    fn new_metric(&self) -> PreAggregatedHistogram {
        PreAggregatedHistogram::default()
    }
}

fn register_exponential_histogram_f64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    histogram: &opentelemetry_sdk::metrics::data::ExponentialHistogram<f64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = histogram.data_points().collect();
    if points.is_empty() {
        return;
    }

    let histogram_family = Family::<DynamicLabels, PreAggregatedHistogram, _>::new_with_constructor(
        PreAggregatedHistogramConstructor,
    );

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        let hist = histogram_family.get_or_create(&labels);
        hist.set_from_exponential(
            point.scale(),
            point.positive_bucket(),
            point.negative_bucket(),
            point.zero_count(),
            point.sum(),
            point.count() as u64,
        );
    }

    let mut container = metrics.lock();
    container
        .histograms
        .insert(name.to_owned(), histogram_family.clone());
    registry.register(name, description, histogram_family);
}

fn register_exponential_histogram_i64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    histogram: &opentelemetry_sdk::metrics::data::ExponentialHistogram<i64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = histogram.data_points().collect();
    if points.is_empty() {
        return;
    }

    let histogram_family = Family::<DynamicLabels, PreAggregatedHistogram, _>::new_with_constructor(
        PreAggregatedHistogramConstructor,
    );

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        let hist = histogram_family.get_or_create(&labels);
        hist.set_from_exponential(
            point.scale(),
            point.positive_bucket(),
            point.negative_bucket(),
            point.zero_count(),
            point.sum() as f64,
            point.count() as u64,
        );
    }

    let mut container = metrics.lock();
    container
        .histograms
        .insert(name.to_owned(), histogram_family.clone());
    registry.register(name, description, histogram_family);
}

fn register_exponential_histogram_u64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    histogram: &opentelemetry_sdk::metrics::data::ExponentialHistogram<u64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = histogram.data_points().collect();
    if points.is_empty() {
        return;
    }

    let histogram_family = Family::<DynamicLabels, PreAggregatedHistogram, _>::new_with_constructor(
        PreAggregatedHistogramConstructor,
    );

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        let hist = histogram_family.get_or_create(&labels);
        hist.set_from_exponential(
            point.scale(),
            point.positive_bucket(),
            point.negative_bucket(),
            point.zero_count(),
            point.sum() as f64,
            point.count() as u64,
        );
    }

    let mut container = metrics.lock();
    container
        .histograms
        .insert(name.to_owned(), histogram_family.clone());
    registry.register(name, description, histogram_family);
}

// Helper functions

fn sanitize_metric_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn create_dynamic_labels(attributes: &[KeyValue]) -> DynamicLabels {
    let mut labels: Vec<(String, String)> = attributes
        .iter()
        .map(|kv| (kv.key.as_str().to_owned(), kv.value.as_str().into_owned()))
        .collect();
    labels.sort_by(|a, b| a.0.cmp(&b.0)); // Ensure consistent ordering
    DynamicLabels(labels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus_client::encoding::text::encode;

    /// Helper: encode a single `PreAggregatedHistogram` registered as "test"
    /// and return the Prometheus text exposition string.
    fn encode_histogram(hist: &PreAggregatedHistogram) -> String {
        let mut registry = Registry::default();
        registry.register("test", "help", hist.clone());
        let mut buf = String::new();
        encode(&mut buf, &registry).unwrap();
        buf
    }

    #[test]
    fn pre_aggregated_histogram_basic_encoding() {
        let hist = PreAggregatedHistogram::default();

        // Scale 0 → base = 2^(2^0) = 2.0
        // offset = 0, counts = [3, 5, 2]
        // Bucket boundaries: (0, 2^1=2], (2, 2^2=4], (4, 2^3=8]
        hist.set_from_raw_buckets(0, 0, &[3, 5, 2], 0, 42.0, 10);

        let output = encode_histogram(&hist);
        assert!(output.contains("test_sum 42.0"), "output: {output}");
        assert!(output.contains("test_count 10"), "output: {output}");
        // Cumulative: le=2 → 3, le=4 → 8, le=8 → 10, le=+Inf → 10
        assert!(
            output.contains(r#"test_bucket{le="2.0"} 3"#),
            "output: {output}"
        );
        assert!(
            output.contains(r#"test_bucket{le="4.0"} 8"#),
            "output: {output}"
        );
        assert!(
            output.contains(r#"test_bucket{le="8.0"} 10"#),
            "output: {output}"
        );
        assert!(
            output.contains(r#"test_bucket{le="+Inf"} 10"#),
            "output: {output}"
        );
    }

    #[test]
    fn pre_aggregated_histogram_with_zero_count() {
        let hist = PreAggregatedHistogram::default();

        // Scale 0, offset 0, one positive bucket with count 2, plus 3 zeros
        hist.set_from_raw_buckets(0, 0, &[2], 3, 5.0, 5);

        let output = encode_histogram(&hist);
        assert!(output.contains("test_count 5"), "output: {output}");
        assert!(output.contains("test_sum 5.0"), "output: {output}");
        // Zeros are added to the first bucket: 2 + 3 = 5
        // Cumulative: le=2 → 5, le=+Inf → 5
        assert!(
            output.contains(r#"test_bucket{le="2.0"} 5"#),
            "output: {output}"
        );
        assert!(
            output.contains(r#"test_bucket{le="+Inf"} 5"#),
            "output: {output}"
        );
    }

    #[test]
    fn pre_aggregated_histogram_zero_only() {
        let hist = PreAggregatedHistogram::default();

        // No positive buckets, only zeros
        hist.set_from_raw_buckets(0, 0, &[], 7, 0.0, 7);

        let output = encode_histogram(&hist);
        assert!(output.contains("test_count 7"), "output: {output}");
        assert!(output.contains("test_sum 0.0"), "output: {output}");
        // Zero-only: creates a (0.0, 7) bucket, then +Inf
        assert!(
            output.contains(r#"test_bucket{le="0.0"} 7"#),
            "output: {output}"
        );
        assert!(
            output.contains(r#"test_bucket{le="+Inf"} 7"#),
            "output: {output}"
        );
    }

    #[test]
    fn pre_aggregated_histogram_with_offset() {
        let hist = PreAggregatedHistogram::default();

        // Scale 0, offset 2, counts = [4]
        // Bucket boundary: base^(2+0+1) = 2^3 = 8
        hist.set_from_raw_buckets(0, 2, &[4], 0, 28.0, 4);

        let output = encode_histogram(&hist);
        assert!(output.contains("test_count 4"), "output: {output}");
        assert!(
            output.contains(r#"test_bucket{le="8.0"} 4"#),
            "output: {output}"
        );
        assert!(
            output.contains(r#"test_bucket{le="+Inf"} 4"#),
            "output: {output}"
        );
    }

    #[test]
    fn pre_aggregated_histogram_finer_scale() {
        let hist = PreAggregatedHistogram::default();

        // Scale 1 → base = 2^(2^(-1)) = 2^0.5 ≈ 1.4142
        // offset 0, counts = [10]
        // Bucket boundary: base^(0+0+1) = √2 ≈ 1.4142
        hist.set_from_raw_buckets(1, 0, &[10], 0, 12.0, 10);

        let output = encode_histogram(&hist);
        assert!(output.contains("test_count 10"), "output: {output}");
        // Check that the bucket upper bound is approximately √2
        // The exact value depends on floating point, so just check the prefix
        assert!(
            output.contains("test_bucket{le=\"1.41421"),
            "expected bucket ≈ √2, output: {output}"
        );
    }
}
