//! Prometheus-specific metric conversion and encoding utilities

#![expect(clippy::cast_possible_wrap)] // u64 -> i64

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use parking_lot::{Mutex, RwLock};
use prometheus_client::encoding::{
    EncodeLabelSet, EncodeMetric, MetricEncoder, NativeHistogram, NativeHistogramBuckets,
    NoLabelSet, prometheus_protobuf,
};
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::{Family, MetricConstructor};
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::{MetricType, TypedMetric};
use prometheus_client::registry::Registry;
use prost::Message as _;

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

/// Encode a Prometheus registry to text format.
///
/// Histograms come out as classic `le` buckets, whose per-series boundaries are not
/// comparable across label sets.
pub fn encode_registry(registry: &Registry) -> Result<String, std::fmt::Error> {
    let mut buffer = String::new();
    prometheus_client::encoding::text::encode(&mut buffer, registry)?;
    Ok(buffer)
}

/// Encode a Prometheus registry to the length-delimited protobuf exposition format.
pub fn encode_registry_protobuf(
    registry: &Registry,
) -> Result<Vec<u8>, prometheus_protobuf::EncodeError> {
    let mut families = prometheus_protobuf::encode(registry)?;

    // `f64::MAX` is `prometheus-client`'s `+Inf` sentinel, honoured only by its text encoder,
    // so the classic buckets have it restored here. Left finite, the sentinel counts as a
    // real bucket and high quantiles resolve into it instead of to the highest real boundary.
    for bucket in families
        .iter_mut()
        .flat_map(|family| family.metric.iter_mut())
        .filter_map(|metric| metric.histogram.as_mut())
        .flat_map(|histogram| histogram.bucket.iter_mut())
    {
        if bucket.upper_bound == f64::MAX {
            bucket.upper_bound = f64::INFINITY;
        }
    }

    let mut encoded = Vec::new();
    for family in families {
        family.encode_length_delimited(&mut encoded)?;
    }
    Ok(encoded)
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
            let metric_name = sanitize_name(metric.name());

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
            .set((point.value() * 1e6) as i64);
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
            .set((point.value() * 1e6) as i64);
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

    native: NativeForm,
}

/// The exponential form of a histogram, as Prometheus encodes it.
#[derive(Debug, Default)]
struct NativeForm {
    /// Prometheus `schema`, which is the `OTel` scale unchanged: the two are the same
    /// quantity with the same definition.
    schema: i32,

    /// Count of observations that fall in the zero bucket.
    zero_count: u64,

    /// `(offset, length)` spans over the occupied positive buckets. The first offset is
    /// absolute, any later one is relative to the end of the previous span.
    spans: Vec<(i32, u32)>,

    /// Bucket counts, each encoded as the difference from the previous bucket.
    deltas: Vec<i64>,
}

impl NativeForm {
    /// `scale` must be in `-4..=8`, or Prometheus rejects the sample.
    fn new(scale: i8, offset: i32, positive_counts: &[u64], zero_count: u64) -> Self {
        re_log::debug_assert!(
            (-4..=8).contains(&scale),
            "scale {scale} is outside the schema range Prometheus accepts"
        );

        // Prometheus bucket `i` covers `(base^(i-1), base^i]` and `OTel` bucket `i` covers
        // `(base^i, base^(i+1)]`, so the same boundary sits one index higher here.
        let mut spans: Vec<(i32, u32)> = Vec::new();
        let mut deltas: Vec<i64> = Vec::new();
        let mut previous = 0_i64;
        let mut covered_through = 0_i32;
        let mut run_start = 0_usize;

        for run in positive_counts.chunk_by(|a, b| (*a == 0) == (*b == 0)) {
            if run.first().is_some_and(|&count| count != 0) {
                for &count in run {
                    deltas.push(count as i64 - previous);
                    previous = count as i64;
                }

                let start_index = offset + 1 + run_start as i32;
                spans.push((start_index - covered_through, run.len() as u32));
                covered_through = start_index + run.len() as i32;
            }
            run_start += run.len();
        }

        Self {
            schema: i32::from(scale),
            zero_count,
            spans,
            deltas,
        }
    }
}

impl PreAggregatedHistogram {
    /// Populate from an `OTel` exponential histogram data point.
    ///
    /// Only positive and zero observations are mapped; negative ones are dropped with a
    /// warning and `count` still includes them. In practice all our histograms track
    /// durations and sizes, which are always non-negative.
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

        let native = NativeForm::new(scale, offset, positive_counts, zero_count);

        let mut inner = self.inner.write();
        inner.sum = sum;
        inner.count = count;
        inner.buckets = buckets;
        inner.native = native;
    }
}

impl Default for PreAggregatedHistogram {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(PreAggregatedHistogramInner {
                sum: 0.0,
                count: 0,
                buckets: Vec::new(),
                native: NativeForm::default(),
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

        encoder.encode_histogram_with_native::<NoLabelSet>(
            inner.sum,
            inner.count,
            &inner.buckets,
            None,
            NativeHistogram {
                schema: inner.native.schema,
                // The `OTel` SDK always reports a zero threshold of 0: only exact zeros count.
                zero_threshold: 0.0,
                zero_count: inner.native.zero_count,
                negative: NativeHistogramBuckets {
                    spans: &[],
                    deltas: &[],
                },
                positive: NativeHistogramBuckets {
                    spans: &inner.native.spans,
                    deltas: &inner.native.deltas,
                },
                created: None,
            },
        )
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

fn sanitize_name(name: &str) -> String {
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
        .map(|kv| {
            (
                sanitize_name(kv.key.as_str()),
                kv.value.as_str().into_owned(),
            )
        })
        .collect();
    labels.sort_by(|a, b| a.0.cmp(&b.0)); // Ensure consistent ordering
    DynamicLabels(labels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus_client::encoding::prometheus_protobuf::prometheus_data_model;
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

    /// Encode a registry into the protobuf data model and return the single histogram in it.
    fn encode_native(hist: &PreAggregatedHistogram) -> prometheus_data_model::Histogram {
        let mut registry = Registry::default();
        registry.register("test", "help", hist.clone());
        let families = prometheus_protobuf::encode(&registry).unwrap();
        families[0].metric[0].histogram.clone().unwrap()
    }

    /// Reconstruct `(bucket_index, count)` pairs the way Prometheus reads the spans back.
    fn decode_native(histogram: &prometheus_data_model::Histogram) -> Vec<(i32, u64)> {
        let mut out = Vec::new();
        let mut index = 0_i32;
        let mut running = 0_i64;
        let mut next_delta = 0_usize;
        for span in &histogram.positive_span {
            index += span.offset;
            for _ in 0..span.length {
                running += histogram.positive_delta[next_delta];
                next_delta += 1;
                assert!(running >= 0, "a bucket count decoded negative");
                out.push((index, running as u64));
                index += 1;
            }
        }
        assert_eq!(
            next_delta,
            histogram.positive_delta.len(),
            "spans and deltas disagree on how many buckets there are"
        );
        out
    }

    /// A finite top bound makes `histogram_quantile` over the classic buckets read ~1e308.
    #[test]
    fn protobuf_classic_buckets_end_at_infinity() {
        let hist = PreAggregatedHistogram::default();
        hist.set_from_raw_buckets(0, 0, &[3, 5], 0, 1.0, 8);

        let mut registry = Registry::default();
        registry.register("test", "help", hist);
        let bytes = encode_registry_protobuf(&registry).unwrap();

        let family =
            prometheus_data_model::MetricFamily::decode_length_delimited(bytes.as_slice()).unwrap();
        let bounds: Vec<f64> = family.metric[0]
            .histogram
            .as_ref()
            .unwrap()
            .bucket
            .iter()
            .map(|bucket| bucket.upper_bound)
            .collect();

        assert!(bounds.last().unwrap().is_infinite());
        assert!(bounds[..bounds.len() - 1].iter().all(|b| b.is_finite()));
    }

    #[test]
    fn native_histogram_round_trip_preserves_grid() {
        let hist = PreAggregatedHistogram::default();
        // Scale 3, first occupied bucket at OTel index 4, counts [3, 0, 5, 2].
        hist.set_from_raw_buckets(3, 4, &[3, 0, 5, 2], 1, 42.0, 11);

        let histogram = encode_native(&hist);

        assert_eq!(histogram.schema, 3, "scale must survive as the schema");
        assert_eq!(histogram.sample_count, 11);
        assert!((histogram.sample_sum - 42.0).abs() < f64::EPSILON);
        assert_eq!(histogram.zero_count, 1);

        // The empty bucket is skipped rather than spanned, so this is two spans.
        assert_eq!(histogram.positive_span.len(), 2);
        assert_eq!(
            (
                histogram.positive_span[0].offset,
                histogram.positive_span[0].length
            ),
            (5, 1)
        );
        assert_eq!(
            (
                histogram.positive_span[1].offset,
                histogram.positive_span[1].length
            ),
            (1, 2)
        );

        // Prometheus bucket indices sit one above the OTel ones for the same boundaries.
        assert_eq!(decode_native(&histogram), vec![(5, 3), (7, 5), (8, 2)]);
    }

    #[test]
    fn native_histogram_round_trips_arbitrary_layouts() {
        let mut seed = 0x5eed_u64;
        let mut next = || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            seed >> 33
        };

        for case in 0..500 {
            let offset = (next() % 101) as i32 - 50;
            let len = 1 + (next() % 20) as usize;
            let counts: Vec<u64> = (0..len)
                .map(|_| if next() % 5 < 2 { 0 } else { 1 + next() % 1000 })
                .collect();

            let hist = PreAggregatedHistogram::default();
            hist.set_from_raw_buckets(2, offset, &counts, 0, 1.0, counts.iter().sum());

            let expected: Vec<(i32, u64)> = counts
                .iter()
                .enumerate()
                .filter(|&(_, &c)| c != 0)
                .map(|(i, &c)| (offset + 1 + i as i32, c))
                .collect();

            assert_eq!(
                decode_native(&encode_native(&hist)),
                expected,
                "case {case}: offset {offset}, counts {counts:?}"
            );
        }
    }

    #[test]
    fn native_histogram_all_empty_buckets_emit_no_span() {
        let hist = PreAggregatedHistogram::default();
        hist.set_from_raw_buckets(0, 3, &[0, 0, 0], 4, 0.0, 4);

        let histogram = encode_native(&hist);
        assert!(histogram.positive_span.is_empty());
        assert!(histogram.positive_delta.is_empty());
        assert_eq!(histogram.zero_count, 4);
    }

    #[test]
    fn native_histogram_without_positive_buckets_emits_no_span() {
        let hist = PreAggregatedHistogram::default();
        hist.set_from_raw_buckets(0, 0, &[], 4, 0.0, 4);

        let histogram = encode_native(&hist);
        assert!(histogram.positive_span.is_empty());
        assert_eq!(histogram.zero_count, 4);
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

    #[test]
    fn escape_dot_in_counter_label() {
        let attrs = vec![KeyValue::new("otel.metric.overflow", "true")];
        let labels = create_dynamic_labels(&attrs);
        let family = Family::<DynamicLabels, Counter>::default();
        family.get_or_create(&labels).inc();

        let mut registry = Registry::default();
        registry.register("test_counter", "help", family);
        let mut buf = String::new();
        encode(&mut buf, &registry).unwrap();

        assert!(buf.contains(r#"test_counter_total{otel_metric_overflow="true"} 1"#));
    }
}
