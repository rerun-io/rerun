//! Everything needed to set up telemetry (logs, traces, metrics) for both clients and servers.

mod args;
mod grpc;

pub use self::{
    args::{LogFormat, TelemetryArgs},
    grpc::{new_grpc_tracing_layer, TracingExtractorInterceptor, TracingInjectorInterceptor},
};

pub mod external {
    pub use opentelemetry;
    pub use tower;
    pub use tower_http;
    pub use tracing;
}

// ---

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer as _};

/// The Redap telemetry pipeline.
///
/// Keep this alive for as long as you need to log, trace and/or measure.
///
/// Will flush everything on drop.
#[derive(Debug, Clone)]
pub struct Telemetry {
    traces: Option<SdkTracerProvider>,
    metrics: Option<SdkMeterProvider>,
}

impl Telemetry {
    pub fn shutdown(&mut self) {
        let Self { traces, metrics } = self;

        // NOTE: We do both `force_flush` and `shutdown` because, even though they both flush the
        // pipeline, sometimes one has better error messages than the other (although, more often
        // than not, they both provide useless errors and you should make sure to look into the
        // DEBUG logs: this is generally where they end up).

        if let Some(traces) = traces {
            if let Err(err) = traces.force_flush() {
                tracing::error!(%err, "failed to flush otel trace provider")
            }
            if let Err(err) = traces.shutdown() {
                tracing::error!(%err, "failed to shutdown otel trace provider")
            }
        }

        if let Some(metrics) = metrics {
            if let Err(err) = metrics.force_flush() {
                tracing::error!(%err, "failed to flush otel metric provider");
            }
            if let Err(err) = metrics.shutdown() {
                tracing::error!(%err, "failed to shutdown otel metric provider");
            }
        }
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl Telemetry {
    #[must_use = "dropping this will flush and shutdown all telemetry systems"]
    pub fn init(args: TelemetryArgs) -> anyhow::Result<Self> {
        let TelemetryArgs {
            disabled,
            service_name,
            attributes,
            log_filter,
            log_format,
            log_closed_spans,
            trace_filter,
            trace_endpoint,
            trace_sampler,
            trace_sampler_args,
            metric_endpoint,
            metric_interval,
        } = args;

        if disabled {
            // TODO(open-telemetry/opentelemetry-rust#1936): must be handled manually at the
            // moment: <https://github.com/open-telemetry/opentelemetry-rust/issues/1936>.

            return Ok(Self {
                metrics: None,
                traces: None,
            });
        }

        // For these things, all we need to do is make sure that the right OTEL env var is set.
        // All the downstream libraries will do the right thing if they are.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("OTEL_SERVICE_NAME", &service_name);
            std::env::set_var("OTEL_RESOURCE_ATTRIBUTES", attributes);
            std::env::set_var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", trace_endpoint);
            std::env::set_var("OTEL_TRACES_SAMPLER", trace_sampler);
            std::env::set_var("OTEL_TRACES_SAMPLER_ARG", trace_sampler_args);
            std::env::set_var("OTEL_EXPORTER_OTLP_METRICS_ENDPOINT", metric_endpoint);
            std::env::set_var("OTEL_METRIC_EXPORT_INTERVAL", metric_interval);
        }

        // TODO: can we grab the logic from rerun?
        // TODO: what do we do with setup_logging?
        let create_filter = |base: &str| {
            Ok::<_, anyhow::Error>(
                EnvFilter::new(base)
                    .add_directive("lance=warn".parse()?)
                    .add_directive("lance-arrow=warn".parse()?)
                    .add_directive("lance-core=warn".parse()?)
                    .add_directive("lance-datafusion=warn".parse()?)
                    .add_directive("lance-encoding=warn".parse()?)
                    .add_directive("lance-file=warn".parse()?)
                    .add_directive("lance-index=warn".parse()?)
                    .add_directive("lance-io=warn".parse()?)
                    .add_directive("lance-linalg=warn".parse()?)
                    .add_directive("lance-table=warn".parse()?),
            )
        };

        // Logging strategy
        // ================
        //
        // * All our logs go through the structured `tracing` macros.
        //
        // * We always log from `tracing` directly into stdio: we never involve the OpenTelemetry
        //   logging API. Production is expected to read the logs from the pod's output.
        //   There is never any internal buffering going on, besides the buffering of stdio itself.
        //
        // * All logs that happen as part of the larger trace/span will automatically be uploaded
        //   with that trace/span.
        //   This makes our traces a very powerful debugging tool, in addition to a profiler.

        let layer_logs_and_traces_stdio = {
            let layer = tracing_subscriber::fmt::layer()
                .with_file(true)
                .with_line_number(true)
                .with_target(false)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_span_events(if log_closed_spans {
                    tracing_subscriber::fmt::format::FmtSpan::CLOSE
                } else {
                    tracing_subscriber::fmt::format::FmtSpan::NONE
                });

            // Everything is typed so we must handle each format in a dedicated branch and make
            // sure we build the final erased object from there.
            let layer = match log_format {
                LogFormat::Pretty => layer
                    .event_format(tracing_subscriber::fmt::format().pretty())
                    .boxed(),

                LogFormat::Compact => layer
                    .event_format(tracing_subscriber::fmt::format().compact())
                    .boxed(),

                LogFormat::Json => layer
                    .event_format(tracing_subscriber::fmt::format().json())
                    .boxed(),
            };

            layer.with_filter(create_filter(&log_filter)?)
        };

        // Tracing strategy
        // ================
        //
        // * All our traces go through the structured `tracing` macros. We *never* use the
        //   OpenTelemetry macros.
        //
        // * The traces go through a first layer of filtering based on the value of `RUST_TRACE`, which
        //   functions similarly to a `RUST_LOG` filter.
        //
        // * The traces are then sent to the OpenTelemetry SDK, where they will go through a pass of
        //   sampling before being sent to the OTLP endpoint.
        //   The sampling mechanism is controlled by the official OTEL environment variables.
        //   span sampling decision.
        //
        // * Spans that contains error logs will properly be marked as failed, and easily findable.

        let (tracer_provider, layer_traces_otlp) = {
            let exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic() // There's no good reason to use HTTP for traces (at the moment, that is)
                .build()?;

            let provider = SdkTracerProvider::builder()
                // // NOTE: Kept around for debugging.
                // .with_simple_exporter(opentelemetry_stdout::SpanExporter::default())
                .with_batch_exporter(exporter)
                .build();

            // This will be used by the `TracingInjectorInterceptor` & `TracingExtractorInterceptor` to
            // encode the trace information into the request headers.
            opentelemetry::global::set_text_map_propagator(
                opentelemetry_sdk::propagation::TraceContextPropagator::new(),
            );

            // This is to make sure that if some third-party system is logging raw OpenTelemetry
            // spans (as opposed to `tracing` spans), we will catch them and forward them
            // appropriately.
            opentelemetry::global::set_tracer_provider(provider.clone());

            let layer = tracing_opentelemetry::layer()
                .with_tracer(provider.tracer(service_name.clone()))
                .with_filter(create_filter(&trace_filter)?);

            (provider, layer)
        };

        // Metric strategy
        // ===============
        //
        // * Our metric strategy is basically the opposite of our logging strategy: everything goes
        //   through OpenTelemetry directly, `tracing` is never involved.
        //
        // * Metrics are uploaded (as opposed to scrapped!) using the OTLP protocol, on a fixed interval
        //   defined by the OTEL_METRIC_EXPORT_INTERVAL environement variable.

        let metric_provider = {
            let exporter = opentelemetry_otlp::MetricExporter::builder()
                // That's the only thing Prometheus supports.
                .with_temporality(opentelemetry_sdk::metrics::Temporality::Cumulative)
                .with_http() // Prometheus only supports HTTP-based OTLP
                .build()?;

            let provider = SdkMeterProvider::builder()
                // NOTE: Kept around for debugging.
                // .with_periodic_exporter(
                //     opentelemetry_stdout::MetricExporterBuilder::default().build(),
                // )
                .with_periodic_exporter(exporter)
                .build();

            // This is to make sure that if some third-party system is logging raw OpenTelemetry
            // metrics (as opposed to `tracing` spans), we will catch them and forward them
            // appropriately.
            //
            //
            // All metrics are logged directly via `opentelemetry`
            opentelemetry::global::set_meter_provider(provider.clone());

            provider
        };

        tracing_subscriber::registry()
            .with(layer_logs_and_traces_stdio)
            .with(layer_traces_otlp)
            .init();

        Ok(Self {
            traces: Some(tracer_provider),
            metrics: Some(metric_provider),
        })
    }
}
