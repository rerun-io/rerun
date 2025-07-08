use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::{
    logs::SdkLoggerProvider, metrics::SdkMeterProvider, trace::SdkTracerProvider,
};
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer as _};

use crate::{LogFormat, TelemetryArgs};

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
    pub fn flush(&mut self) {
        let Self {
            logs,
            traces,
            metrics,
            drop_behavior: _,
        } = self;

        if let Some(logs) = logs {
            if let Err(err) = logs.force_flush() {
                tracing::error!(%err, "failed to flush otel log provider");
            }
        }

        if let Some(traces) = traces {
            if let Err(err) = traces.force_flush() {
                tracing::error!(%err, "failed to flush otel trace provider");
            }
        }

        if let Some(metrics) = metrics {
            if let Err(err) = metrics.force_flush() {
                tracing::error!(%err, "failed to flush otel metric provider");
            }
        }
    }

    pub fn shutdown(&mut self) {
        // NOTE: We do both `force_flush` and `shutdown` because, even though they both flush the
        // pipeline, sometimes one has better error messages than the other (although, more often
        // than not, they both provide useless errors and you should make sure to look into the
        // DEBUG logs: this is generally where they end up).
        self.flush();

        let Self {
            logs,
            traces,
            metrics,
            drop_behavior: _,
        } = self;

        if let Some(logs) = logs {
            if let Err(err) = logs.shutdown() {
                tracing::error!(%err, "failed to shutdown otel log provider");
            }
        }

        if let Some(traces) = traces {
            if let Err(err) = traces.shutdown() {
                tracing::error!(%err, "failed to shutdown otel trace provider");
            }
        }

        if let Some(metrics) = metrics {
            if let Err(err) = metrics.shutdown() {
                tracing::error!(%err, "failed to shutdown otel metric provider");
            }
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
            metric_endpoint,
            metric_interval,
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
                drop_behavior,
            });
        }

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
            // TODO(zehiko) is there a better way to do this?!
            // Lance traces are all at INFO level and they are over 30% of total spans, hence
            // disabling them if we're at the INFO level cause we only want them at DEBUG
            let lance_level = if base == "info" { "off" } else { base };

            Ok::<_, anyhow::Error>(
                EnvFilter::new(base)
                    // TODO(cmc): do not override user's choice, bring back the logic from re_log
                    .add_directive(format!("aws_smithy_runtime={forced}").parse()?)
                    .add_directive(format!("datafusion={forced}").parse()?)
                    .add_directive(format!("datafusion_optimizer={forced}").parse()?)
                    .add_directive(format!("h2={forced}").parse()?)
                    .add_directive(format!("hyper={forced}").parse()?)
                    .add_directive(format!("hyper_util={forced}").parse()?)
                    .add_directive(format!("lance-arrow={forced}").parse()?)
                    .add_directive(format!("lance-core={forced}").parse()?)
                    .add_directive(format!("lance-datafusion={forced}").parse()?)
                    .add_directive(format!("lance-encoding={forced}").parse()?)
                    .add_directive(format!("lance-file={forced}").parse()?)
                    .add_directive(format!("lance-index={forced}").parse()?)
                    .add_directive(format!("lance-io={forced}").parse()?)
                    .add_directive(format!("lance-linalg={forced}").parse()?)
                    .add_directive(format!("lance-table={forced}").parse()?)
                    .add_directive(format!("lance={forced}").parse()?)
                    .add_directive(format!("opentelemetry-otlp={forced}").parse()?)
                    .add_directive(format!("opentelemetry={forced}").parse()?)
                    .add_directive(format!("opentelemetry_sdk={forced}").parse()?)
                    .add_directive(format!("rustls={forced}").parse()?)
                    .add_directive(format!("sqlparser={forced}").parse()?)
                    .add_directive(format!("tonic={forced}").parse()?)
                    .add_directive(format!("tonic_web={forced}").parse()?)
                    .add_directive(format!("tower={forced}").parse()?)
                    .add_directive(format!("tower_http={forced}").parse()?)
                    .add_directive(format!("tower_web={forced}").parse()?)
                    //
                    .add_directive(format!("lance::index={lance_level}").parse()?)
                    .add_directive(format!("lance::dataset::scanner={lance_level}").parse()?)
                    .add_directive(format!("lance::dataset::builder={lance_level}").parse()?)
                    .add_directive("lance_encoding=off".parse()?), // this one is a real nightmare
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
                    let layer = layer.event_format(tracing_subscriber::fmt::format().$format());
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
                .build()?;

            let provider = SdkTracerProvider::builder()
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
                .with_filter(create_filter(&trace_filter, "info")?)
                .boxed();

            (Some(provider), Some(layer))
        } else {
            (None, None)
        };

        // Metric strategy
        // ===============
        //
        // * Our metric strategy is basically the opposite of our logging strategy: everything goes
        //   through OpenTelemetry directly, `tracing` is never involved.
        //
        // * Metrics are uploaded (as opposed to scrapped!) using the OTLP protocol, on a fixed interval
        //   defined by the OTEL_METRIC_EXPORT_INTERVAL environment variable.

        let metric_provider = if otel_enabled {
            let exporter = opentelemetry_otlp::MetricExporter::builder()
                // That's the only thing Prometheus supports.
                .with_temporality(opentelemetry_sdk::metrics::Temporality::Cumulative)
                .with_http() // Prometheus only supports HTTP-based OTLP
                .build()?;

            let provider = SdkMeterProvider::builder()
                .with_periodic_exporter(exporter)
                .build();

            // All metrics are logged directly via `opentelemetry`.
            opentelemetry::global::set_meter_provider(provider.clone());

            Some(provider)
        } else {
            None
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
                .try_init()?;
        }

        Ok(Self {
            drop_behavior,
            logs: logger_provider,
            traces: tracer_provider,
            metrics: metric_provider,
        })
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
