//! Prometheus-specific metric conversion and encoding utilities

#![expect(clippy::cast_possible_wrap)] // u64 -> i64

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Arc;

use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use parking_lot::Mutex;
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::{Histogram, exponential_buckets};
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
    pub histograms: HashMap<String, Family<DynamicLabels, Histogram>>,
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
                AggregatedMetrics::F64(MetricData::Histogram(histogram)) => {
                    register_histogram_f64(
                        &mut registry,
                        &metric_name,
                        metric.description(),
                        histogram,
                        metrics,
                    );
                }
                AggregatedMetrics::I64(MetricData::Histogram(histogram)) => {
                    register_histogram_i64(
                        &mut registry,
                        &metric_name,
                        metric.description(),
                        histogram,
                        metrics,
                    );
                }
                AggregatedMetrics::U64(MetricData::Histogram(histogram)) => {
                    register_histogram_u64(
                        &mut registry,
                        &metric_name,
                        metric.description(),
                        histogram,
                        metrics,
                    );
                }
                _ => {
                    // ExponentialHistogram or other types not supported
                }
            }
            // Note: ExponentialHistogram is not directly supported in Prometheus
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

fn register_histogram_f64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    histogram: &opentelemetry_sdk::metrics::data::Histogram<f64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = histogram.data_points().collect();
    if points.is_empty() {
        return;
    }

    // Create histogram with default exponential buckets
    // TODO(thz): Consider preserving original bucket boundaries if needed
    let histogram_family = Family::<DynamicLabels, Histogram>::new_with_constructor(|| {
        Histogram::new(exponential_buckets(0.005, 2.0, 10))
    });

    // Note: We can't directly set histogram values in prometheus-client,
    // we can only observe individual samples. Since we have pre-aggregated data,
    // we'll need to approximate by observing samples.
    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        let hist = histogram_family.get_or_create(&labels);

        // Approximate by observing the mean value multiple times
        if point.count() > 0 {
            let mean = point.sum() / point.count() as f64;
            // Observe the mean value to approximate the distribution
            // This preserves sum but not the exact distribution
            for _ in 0..point.count() {
                hist.observe(mean);
            }
        }
    }

    let mut container = metrics.lock();
    container
        .histograms
        .insert(name.to_owned(), histogram_family.clone());
    registry.register(name, description, histogram_family);
}

fn register_histogram_i64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    histogram: &opentelemetry_sdk::metrics::data::Histogram<i64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = histogram.data_points().collect();
    if points.is_empty() {
        return;
    }

    let histogram_family = Family::<DynamicLabels, Histogram>::new_with_constructor(|| {
        Histogram::new(exponential_buckets(0.005, 2.0, 10))
    });

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        let hist = histogram_family.get_or_create(&labels);

        if point.count() > 0 {
            let mean = point.sum() as f64 / point.count() as f64;
            for _ in 0..point.count() {
                hist.observe(mean);
            }
        }
    }

    let mut container = metrics.lock();
    container
        .histograms
        .insert(name.to_owned(), histogram_family.clone());
    registry.register(name, description, histogram_family);
}

fn register_histogram_u64(
    registry: &mut Registry,
    name: &str,
    description: &str,
    histogram: &opentelemetry_sdk::metrics::data::Histogram<u64>,
    metrics: &Arc<Mutex<MetricContainer>>,
) {
    let points: Vec<_> = histogram.data_points().collect();
    if points.is_empty() {
        return;
    }

    let histogram_family = Family::<DynamicLabels, Histogram>::new_with_constructor(|| {
        Histogram::new(exponential_buckets(0.005, 2.0, 10))
    });

    for point in &points {
        let attrs: Vec<_> = point.attributes().cloned().collect();
        let labels = create_dynamic_labels(&attrs);
        let hist = histogram_family.get_or_create(&labels);

        if point.count() > 0 {
            let mean = point.sum() as f64 / point.count() as f64;
            for _ in 0..point.count() {
                hist.observe(mean);
            }
        }
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
