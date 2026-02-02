use std::sync::Arc;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithTonicConfig as _;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::{BatchConfigBuilder, BatchSpanProcessor, SdkTracerProvider};
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer as _};

use crate::shared_reader::SharedManualReader;
use crate::{LogFormat, TelemetryArgs, TraceIdLayer};

// ---

/// The Redap telemetry pipeline.
///
/// Keep this alive for as long as you need to log, trace and/or measure.
///
/// Will flush everything on drop.
#[derive(Debug, Clone)]
pub struct Telemetry {
    logs: Option<SdkLoggerProvider>,
    traces: Option<SdkTracerProvider>,
    metrics: Option<SdkMeterProvider>,

    /// The shared manual reader for pull-based metrics collection
    metrics_reader: Option<Arc<opentelemetry_sdk::metrics::ManualReader>>,

    drop_behavior: TelemetryDropBehavior,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum TelemetryDropBehavior {
    /// The telemetry pipeline will be flushed everytime a [`Telemetry`] is dropped.
    ///
    /// This is particularly useful to use in conjunction with the fact that [`Telemetry`]
    /// is `Clone`: lazy initialize a [`Telemetry`] into a static `LazyCell`/`LazyLock`, and keep
    /// returning clones of that value.
    /// You are guaranteed that the pipeline will get flushed everytime one of these clone goes out
    /// of scope.
    Flush,

    /// The telemetry pipeline will be flushed and shutdown the first time a [`Telemetry`] is dropped.
    ///
    /// The pipeline is then inactive, and all logs, traces and metrics are dropped.
    #[default]
    Shutdown,
}

impl Telemetry {
    pub fn flush(&self) {
        let Self {
            logs,
            traces,
            metrics,
            metrics_reader: _,
            drop_behavior: _,
        } = self;

        if let Some(logs) = logs
            && let Err(err) = logs.force_flush()
        {
            tracing::error!(%err, "failed to flush otel log provider");
        }

        if let Some(traces) = traces
            && let Err(err) = traces.force_flush()
        {
            tracing::error!(%err, "failed to flush otel trace provider");
        }

        if let Some(metrics) = metrics
            && let Err(err) = metrics.force_flush()
        {
            tracing::error!(%err, "failed to flush otel metric provider");
        }
    }

    pub fn shutdown(&self) {
        // NOTE: We do both `force_flush` and `shutdown` because, even though they both flush the
        // pipeline, sometimes one has better error messages than the other (although, more often
        // than not, they both provide useless errors and you should make sure to look into the
        // DEBUG logs: this is generally where they end up).
        self.flush();

        let Self {
            logs,
            traces,
            metrics,
            metrics_reader: _,
            drop_behavior: _,
        } = self;

        if let Some(logs) = logs
            && let Err(err) = logs.shutdown()
        {
            tracing::error!(%err, "failed to shutdown otel log provider");
        }

        if let Some(traces) = traces
            && let Err(err) = traces.shutdown()
        {
            tracing::error!(%err, "failed to shutdown otel trace provider");
        }

        if let Some(metrics) = metrics
            && let Err(err) = metrics.shutdown()
        {
            tracing::error!(%err, "failed to shutdown otel metric provider");
        }
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        match self.drop_behavior {
            TelemetryDropBehavior::Flush => self.flush(),
            TelemetryDropBehavior::Shutdown => self.shutdown(),
        }
    }
}

impl Telemetry {
    #[must_use = "dropping this will flush and shutdown all telemetry systems"]
    pub fn init(args: TelemetryArgs, drop_behavior: TelemetryDropBehavior) -> anyhow::Result<Self> {
        let TelemetryArgs {
            tracy_enabled,
            enabled,
            otel_enabled,
            service_name,
            attributes,
            log_filter,
            log_test_output,
            log_format,
            log_closed_spans,
            log_otlp_enabled,
            log_endpoint,
            trace_filter,
            trace_endpoint,
            trace_sampler,
            trace_sampler_args,
            tracestate,
            metric_endpoint,
            metric_interval,
            metrics_listen_address: _, // TelemetryArgs only, used at the caller site
        } = args;

        if !enabled {
            if tracy_enabled {
                #[cfg(feature = "tracy")]
                {
                    tracing_subscriber::registry()
                        .with(self::tracy::tracy_layer())
                        .try_init()?;
                }

                #[cfg(not(feature = "tracy"))]
                {
                    anyhow::bail!(
                        "`TRACY_ENABLED=true` but the 'tracy' feature flag is not toggled"
                    );
                }
            }

            return Ok(Self {
                logs: None,
                metrics: None,
                traces: None,
                metrics_reader: None,
                drop_behavior,
            });
        }

        let Some(service_name) = service_name else {
            anyhow::bail!(
                "either `OTEL_SERVICE_NAME` or `TelemetryArgs::service_name` must be set in order to initialize telemetry"
            );
        };

        // For these things, all we need to do is make sure that the right OTEL env var is set.
        // All the downstream libraries will do the right thing if they are.
        //
        // Safety: anything touching the env is unsafe, tis what it is.
        #[expect(unsafe_code)]
        unsafe {
            std::env::set_var("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", log_endpoint);
            std::env::set_var("OTEL_EXPORTER_OTLP_METRICS_ENDPOINT", metric_endpoint);
            std::env::set_var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", trace_endpoint);
            std::env::set_var("OTEL_METRIC_EXPORT_INTERVAL", metric_interval);
            std::env::set_var("OTEL_RESOURCE_ATTRIBUTES", attributes);
            std::env::set_var("OTEL_SERVICE_NAME", &service_name);
            std::env::set_var("OTEL_TRACES_SAMPLER", trace_sampler);
            std::env::set_var("OTEL_TRACES_SAMPLER_ARG", trace_sampler_args);
        }

        let create_filter = |base: &str, forced: &str| {
            use crate::EnvFilterExt as _;

            EnvFilter::new(base)
                .add_directive_if_absent(base, "aws_smithy_runtime", forced)?
                .add_directive_if_absent(base, "datafusion", forced)?
                .add_directive_if_absent(base, "datafusion_optimizer", forced)?
                .add_directive_if_absent(base, "h2", forced)?
                .add_directive_if_absent(base, "hyper", forced)?
                .add_directive_if_absent(base, "hyper_util", forced)?
                .add_directive_if_absent(base, "lance", forced)?
                .add_directive_if_absent(base, "lance-arrow", forced)?
                .add_directive_if_absent(base, "lance-core", forced)?
                .add_directive_if_absent(base, "lance-datafusion", forced)?
                .add_directive_if_absent(base, "lance-encoding", forced)?
                .add_directive_if_absent(base, "lance-file", forced)?
                .add_directive_if_absent(base, "lance-index", forced)?
                .add_directive_if_absent(base, "lance-io", forced)?
                .add_directive_if_absent(base, "lance-linalg", forced)?
                .add_directive_if_absent(base, "lance-table", forced)?
                .add_directive_if_absent(base, "lance", forced)?
                .add_directive_if_absent(base, "opentelemetry-otlp", forced)?
                .add_directive_if_absent(base, "opentelemetry", forced)?
                .add_directive_if_absent(base, "opentelemetry_sdk", forced)?
                .add_directive_if_absent(base, "rustls", forced)?
                .add_directive_if_absent(base, "sqlparser", forced)?
                .add_directive_if_absent(base, "tonic", forced)?
                .add_directive_if_absent(base, "tonic_web", forced)?
                .add_directive_if_absent(base, "tower", forced)?
                .add_directive_if_absent(base, "tower_http", forced)?
                .add_directive_if_absent(base, "tower_web", forced)?
                .add_directive_if_absent(base, "typespec_client_core", forced)?
                //
                .add_directive_if_absent(base, "lance::index", "off")?
                .add_directive_if_absent(base, "lance::io::exec", "off")?
                .add_directive_if_absent(base, "lance::dataset::scanner", "off")?
                .add_directive_if_absent(base, "lance_index", "off")?
                .add_directive_if_absent(base, "lance::dataset::builder", "off")?
                .add_directive_if_absent(base, "lance_encoding", "off")
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
        //
        // * If `OTEL_EXPORTER_OTLP_LOGS_ENABLED=true`, all logs will be forwarded to an OpenTelemetry
        //   collector in addition to standard IO.

        let layer_logs_and_traces_stdio = {
            let layer = tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
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

            // Everything is generically typed, which is why this is such a nightmare to do.
            macro_rules! handle_format {
                ($format:ident) => {{
                    let layer = layer.$format();
                    if log_test_output {
                        layer.with_test_writer().boxed()
                    } else {
                        layer.boxed()
                    }
                }};
            }
            let layer = match log_format {
                LogFormat::Pretty => handle_format!(pretty),
                LogFormat::Compact => handle_format!(compact),
                LogFormat::Json => handle_format!(json),
            };

            layer.with_filter(create_filter(&log_filter, "warn")?)
        };

        let (logger_provider, layer_logs_otlp) = if otel_enabled && log_otlp_enabled {
            use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;

            let exporter = opentelemetry_otlp::LogExporter::builder()
                .with_tonic() // There's no good reason to use HTTP for logs (at the moment, that is)
                .build()?;

            let provider = SdkLoggerProvider::builder()
                .with_batch_exporter(exporter)
                .build();

            let layer = OpenTelemetryTracingBridge::new(&provider).boxed();

            (
                Some(provider),
                Some(layer.with_filter(create_filter(&log_filter, "warn")?)),
            )
        } else {
            (None, None)
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
        //
        // * Spans that contains error logs will properly be marked as failed, and easily findable.

        let (tracer_provider, layer_traces_otlp) = if otel_enabled {
            let exporter = opentelemetry_otlp::SpanExporter::builder()
                .with_tonic() // There's no good reason to use HTTP for traces (at the moment, that is)
                .with_compression(opentelemetry_otlp::Compression::Gzip) // use gzip compression to reduce bandwidth
                .build()?;

            // we customize batch exporter config to ensure more optimal span exporting
            let batch_config = BatchConfigBuilder::default()
                // increase max queue size from default 2048 to ensure we don't drop spans during high throughput
                .with_max_queue_size(8192)
                // export more spans per batch to reduce number of requests (default is 512)
                // together with queue size this help ensure more robust exporting under high throughput
                .with_max_export_batch_size(2048)
                .build();

            let batch_processor = BatchSpanProcessor::builder(exporter)
                .with_batch_config(batch_config)
                .build();

            let provider = SdkTracerProvider::builder()
                .with_span_processor(batch_processor)
                .build();

            // This will be used by the `TracingInjectorInterceptor` to encode the trace information into the request headers.
            // Additional `tracestate` can be added through the relevant env var and the custom enricher below.
            let mut propagators: Vec<
                Box<dyn opentelemetry::propagation::TextMapPropagator + Send + Sync>,
            > = vec![Box::new(
                opentelemetry_sdk::propagation::TraceContextPropagator::new(),
            )];

            if !tracestate.is_empty() {
                let enricher = crate::tracestate::TraceStateEnricher::new(&tracestate);
                propagators.push(Box::new(enricher));
            }

            opentelemetry::global::set_text_map_propagator(
                opentelemetry::propagation::TextMapCompositePropagator::new(propagators),
            );

            // This is to make sure that if some third-party system is logging raw OpenTelemetry
            // spans (as opposed to `tracing` spans), we will catch them and forward them
            // appropriately.
            opentelemetry::global::set_tracer_provider(provider.clone());

            let layer = tracing_opentelemetry::layer()
                .with_tracer(provider.tracer(service_name.clone()))
                .with_filter(create_filter(&trace_filter, "info")?)
                .boxed();

            (Some(provider), Some(layer))
        } else {
            (None, None)
        };

        // Metric strategy
        // ===============
        //
        // * Metrics can be pushed to an OTLP endpoint as defined by OTEL SDK variables.
        //   OTEL_METRIC_EXPORT_INTERVAL environment variable applies for push interval.
        //   This is enabled by setting OTEL_EXPORTER_OTLP_METRICS_ENDPOINT
        //
        // * Additionally a prometheus-style scraping endpoint can be enabled by calling
        //   start_metrics_listener() on the returned Telemetry instance.
        //
        // Both ways use the same data for actual metrics.
        //
        let (metric_provider, metrics_reader) = if otel_enabled {
            let mut builder = SdkMeterProvider::builder();

            // OTLP exporter for push-based metrics
            let otlp_exporter = opentelemetry_otlp::MetricExporter::builder()
                .with_temporality(opentelemetry_sdk::metrics::Temporality::Cumulative)
                .with_http()
                .build()?;
            builder = builder.with_periodic_exporter(otlp_exporter);

            // Always add a ManualReader for potential metrics listener
            // We use SharedManualReader to share the same reader instance between
            // the MeterProvider (for registration) and the metrics server (for collection)
            let shared_reader =
                SharedManualReader::new(opentelemetry_sdk::metrics::Temporality::Cumulative);

            let reader_for_telemetry = shared_reader.inner();
            builder = builder.with_reader(shared_reader);

            let provider = builder.build();

            // Set as global provider - this makes all metrics created via opentelemetry::global::meter()
            // available to all registered readers: OTLP push and ManualReader
            opentelemetry::global::set_meter_provider(provider.clone());

            tracing::info!("metric provider created with manual reader support");

            (Some(provider), Some(reader_for_telemetry))
        } else {
            (None, None)
        };

        if tracy_enabled {
            #[cfg(feature = "tracy")]
            {
                tracing::warn!(
                    "using tracy in addition to standard telemetry stack, consider `TELEMETRY_ENABLED=false`"
                );

                tracing_subscriber::registry()
                    .with(layer_logs_otlp)
                    .with(layer_logs_and_traces_stdio)
                    .with(layer_traces_otlp)
                    .with(TraceIdLayer::default())
                    .with(self::tracy::tracy_layer())
                    .try_init()?;
            }

            #[cfg(not(feature = "tracy"))]
            {
                anyhow::bail!("`TRACY_ENABLED=true` but the 'tracy' feature flag is not toggled");
            }
        } else {
            tracing_subscriber::registry()
                .with(layer_logs_otlp)
                .with(layer_logs_and_traces_stdio)
                .with(layer_traces_otlp)
                .with(TraceIdLayer::default())
                .try_init()?;
        }

        crate::memory_telemetry::install_memory_use_meters();

        tracing::info!("Telemetry initialized");

        Ok(Self {
            drop_behavior,
            logs: logger_provider,
            traces: tracer_provider,
            metrics: metric_provider,
            metrics_reader,
        })
    }

    /// Start a dedicated HTTP server for metrics collection at the given address.
    ///
    /// This binds to the specified address and spawns an HTTP server that exposes a
    /// `/metrics` endpoint for Prometheus-style scraping. The metrics are collected
    /// on-demand when the endpoint is accessed.
    ///
    /// # Arguments
    ///
    /// * `addr` - The address to listen on (e.g., ":9091", "0.0.0.0:9091", or "127.0.0.1:9091")
    ///
    /// # Returns
    ///
    /// Returns an error if:
    /// - Telemetry was not initialized with metrics support
    /// - The address is invalid or cannot be parsed
    /// - The server fails to bind to the address (e.g., port already in use)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use re_perf_telemetry::{Telemetry, TelemetryArgs, TelemetryDropBehavior};
    ///
    /// let args = TelemetryArgs { /* ... */ };
    /// let telemetry = Telemetry::init(args, TelemetryDropBehavior::Shutdown)?;
    ///
    /// // This will return an error if the port is already in use
    /// telemetry.start_metrics_listener(":9091").await?;
    /// ```
    pub async fn start_metrics_listener(&self, addr: &str) -> anyhow::Result<()> {
        let reader = self.metrics_reader.as_ref()
            .ok_or_else(|| anyhow::anyhow!(
                "Cannot start metrics listener: telemetry was not initialized with metrics support. \
                Ensure TELEMETRY_ENABLED=true and OTEL_SDK_ENABLED=true"
            ))?;

        // Clone the Arc to pass to the server
        let reader_for_server = Arc::clone(reader);

        // Start the metrics server - this will bind synchronously and return an error
        // if binding fails (e.g., port already in use), but the actual serving happens
        // asynchronously in a spawned task
        crate::metrics_server::start_metrics_server(addr, reader_for_server).await?;

        Ok(())
    }
}

// ---

/// Tracy integration
/// =================
///
/// * Use `TRACY_ENABLED=true` in combination with `tracy` feature flag.
/// * The Tracy Viewer version must match the client's: we use 0.12 for both (latest as of this writing).
///
/// See <https://github.com/wolfpld/tracy>.
///
/// ⚠️Tracy will start monitoring OS performance as soon as the client library is loaded in!
/// This is very cheap, but make sure to disable the `tracy` feature flag if that turns out to be a
/// problem for whatever reason (`TRACY_ENABLED=false`) won't cut it.
///
/// ⚠️Keep in mind that the `Counts` that are displayed in Tracy account for every yields!
/// E.g. an async function that yields 50 times will be counted as 51 (the first call + 50 yields).
#[cfg(feature = "tracy")]
mod tracy {
    #[derive(Default)]
    pub struct TracyConfig(tracing_subscriber::fmt::format::DefaultFields);

    impl tracing_tracy::Config for TracyConfig {
        type Formatter = tracing_subscriber::fmt::format::DefaultFields;

        fn formatter(&self) -> &Self::Formatter {
            &self.0
        }

        fn format_fields_in_zone_name(&self) -> bool {
            false
        }
    }

    pub fn tracy_layer() -> tracing_tracy::TracyLayer<TracyConfig> {
        tracing_tracy::TracyLayer::new(TracyConfig::default())
    }
}
