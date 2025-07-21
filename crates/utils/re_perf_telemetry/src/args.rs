#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogFormat {
    Pretty,
    Compact,
    Json,
}

impl std::fmt::Display for LogFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Pretty => "pretty",
            Self::Compact => "compact",
            Self::Json => "json",
        })
    }
}

impl std::str::FromStr for LogFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "pretty" => Self::Pretty,
            "compact" => Self::Compact,
            "json" => Self::Json,
            unknown => anyhow::bail!("unknown LogFormat: '{unknown}"),
        })
    }
}

// ---

const fn default_telemetry_attributes() -> &'static str {
    concat!(
        "service.namespace=redap,service.version=",
        env!("CARGO_PKG_VERSION")
    )
}

const fn default_log_filter() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "info"
    }
}

/// Complete configuration for all things telemetry.
///
/// Many of these are part of the official `OpenTelemetry` spec and can be configured directly via
/// the environment. Refer to this command's help as well as [the spec].
///
/// [the spec]: https://opentelemetry.io/docs/specs/otel/configuration/sdk-environment-variables/
#[derive(Clone, Debug, clap::Parser)]
#[clap(author, version, about)]
pub struct TelemetryArgs {
    /// Enable telemetry?
    ///
    /// If disabled, this will completely skip the initialization of the different telemetry subscribers,
    /// both native and `OpenTelemetry`.
    /// i.e. all events will be dropped immediately, with very minimal cost.
    /// Disabling is particularly useful in conjunction with `TRACY_ENABLED`, to prevent noise in the trace
    /// data.
    ///
    /// To remove all traces of telemetry _at compile time_, compile with the appropriate `tracing`
    /// feature flags instead: <https://docs.rs/tracing/0.1.41/tracing/level_filters/index.html>.
    #[cfg_attr(
        feature = "enabled",
        clap(long, env = "TELEMETRY_ENABLED", default_value_t = true)
    )]
    #[cfg_attr(
        not(feature = "enabled"),
        clap(long, env = "TELEMETRY_ENABLED", default_value_t = false)
    )]
    pub enabled: bool,

    /// If set, all the traces and logs will be forwarded to [Tracy], without any filtering.
    ///
    /// It is recommended to set `TELEMETRY_ENABLED=false` when using this, to prevent the noise
    /// from the rest of the `tracing` stack of interfering with your measurements.
    ///
    /// This requires the `tracy` feature flag.
    ///
    /// [Tracy]: https://github.com/wolfpld/tracy
    #[cfg_attr(
        feature = "tracy_enabled",
        clap(long, env = "TRACY_ENABLED", default_value_t = true)
    )]
    #[cfg_attr(
        not(feature = "tracy_enabled"),
        clap(long, env = "TRACY_ENABLED", default_value_t = false)
    )]
    pub tracy_enabled: bool,

    /// Enable `OpenTelemetry`?
    ///
    /// This will initialize all the different `OpenTelemetry` subscribers, so that the data gets
    /// uploaded to OTLP-compatible external services.
    ///
    /// The base telemetry in and of itself will keep working even if this is disabled. E.g. logs
    /// will be forwarded to standard IO regardless.
    ///
    /// This has no effect if `TELEMETRY_ENABLED` is false.
    #[cfg_attr(
        feature = "otel_enabled",
        clap(long, env = "OTEL_SDK_ENABLED", default_value_t = true)
    )]
    #[cfg_attr(
        not(feature = "otel_enabled"),
        clap(long, env = "OTEL_SDK_ENABLED", default_value_t = false)
    )]
    pub otel_enabled: bool,

    /// The service name used for all things telemetry.
    ///
    /// Part of the `OpenTelemetry` spec.
    #[clap(long, env = "OTEL_SERVICE_NAME")]
    pub service_name: String,

    /// The service attributes used for all things telemetry.
    ///
    /// Expects a comma-separated string of key=value pairs, e.g. `a=b,c=d`.
    ///
    /// Part of the `OpenTelemetry` spec.
    #[clap(
        long,
        env = "OTEL_RESOURCE_ATTRIBUTES",
        default_value = default_telemetry_attributes(),
    )]
    pub attributes: String,

    /// This is the same as `RUST_LOG`.
    ///
    /// This only affects logs, not traces nor metrics.
    #[clap(long, env = "RUST_LOG", default_value_t = default_log_filter().to_owned())]
    pub log_filter: String,

    /// Capture test output as part of the logs.
    #[clap(long, env = "RUST_LOG_CAPTURE_TEST_OUTPUT", default_value_t = false)]
    pub log_test_output: bool,

    /// Use `json` in production. Pick between `pretty` and `compact` during development according
    /// to your preferences.
    #[clap(long, env = "RUST_LOG_FORMAT", default_value_t = LogFormat::Pretty)]
    pub log_format: LogFormat,

    /// If true, log extra information about all retired spans, including their timings.
    #[clap(long, env = "RUST_LOG_CLOSED_SPANS", default_value_t = false)]
    pub log_closed_spans: bool,

    /// Should an OTLP exporter for logs be setup too (in addition to trace events)?
    ///
    /// *Not* part of the `OpenTelemetry` spec.
    ///
    /// See also [`Self::log_endpoint`].
    #[clap(long, env = "OTEL_EXPORTER_OTLP_LOGS_ENABLED", default_value_t = false)]
    pub log_otlp_enabled: bool,

    /// The gRPC OTLP endpoint to send the logs to.
    ///
    /// It's fine for the target endpoint to be down.
    ///
    /// Part of the `OpenTelemetry` spec.
    #[clap(
        long,
        env = "OTEL_EXPORTER_OTLP_LOGS_ENDPOINT",
        default_value = "http://localhost:4317"
    )]
    pub log_endpoint: String,

    /// Same as `RUST_LOG`, but for traces.
    ///
    /// This only affects traces, not logs nor metrics.
    #[clap(long, env = "RUST_TRACE", default_value = "info")]
    pub trace_filter: String,

    /// The gRPC OTLP endpoint to send the traces to.
    ///
    /// It's fine for the target endpoint to be down.
    ///
    /// Part of the `OpenTelemetry` spec.
    #[clap(
        long,
        env = "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
        default_value = "http://localhost:4317"
    )]
    pub trace_endpoint: String,

    /// How are spans sampled?
    ///
    /// This is applied _after_ `RUST_TRACE`.
    ///
    /// Remember: sampling only applies at the `OpenTelemetry` level, i.e. we are sampling the
    /// traces we export, *not* the traces we generate. Internally, all traces are always
    /// generated, there is no such thing as sampling at the `tracing` level.
    ///
    /// Part of the `OpenTelemetry` spec.
    #[clap(
        long,
        env = "OTEL_TRACES_SAMPLER",
        default_value = "parentbased_traceidratio"
    )]
    pub trace_sampler: String,

    /// The specified value will only be used if `OTEL_TRACES_SAMPLER` is set.
    ///
    /// Each Sampler type defines its own expected input, if any. Invalid or unrecognized input
    /// MUST be logged and MUST be otherwise ignored, i.e. the implementation MUST behave as if
    /// `OTEL_TRACES_SAMPLER_ARG` is not set.
    ///
    /// Part of the `OpenTelemetry` spec.
    #[clap(long, env = "OTEL_TRACES_SAMPLER_ARG", default_value = "1.0")]
    pub trace_sampler_args: String,

    /// The HTTP OTLP endpoint to send the metrics to.
    ///
    /// It's fine for the target endpoint to be down.
    ///
    /// Part of the `OpenTelemetry` spec.
    #[clap(
        long,
        env = "OTEL_EXPORTER_OTLP_METRICS_ENDPOINT",
        default_value = "http://localhost:9090/api/v1/otlp/v1/metrics"
    )]
    pub metric_endpoint: String,

    /// The interval in milliseconds at which metrics are pushed to the collector.
    ///
    /// Part of the `OpenTelemetry` spec.
    #[clap(long, env = "OTEL_METRIC_EXPORT_INTERVAL", default_value = "10000")]
    pub metric_interval: String,
}
