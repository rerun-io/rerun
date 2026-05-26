use std::sync::Arc;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithTonicConfig as _;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::{Aggregation, SdkMeterProvider};
use opentelemetry_sdk::trace::{BatchConfigBuilder, BatchSpanProcessor, SdkTracerProvider};
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Layer as _};

use crate::shared_reader::SharedManualReader;
use crate::trace_id_format::TraceIdFormat;
use crate::{LogFormat, SpanMetadataCleanupLayer, TelemetryArgs};

const OTLP_EXPORTER_ENV_VAR: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";

/// Resolved trace destinations for `Telemetry::init`. Each field is
/// `Some(url)` iff the corresponding exporter should be built. The two
/// fields are independent — both, either, or neither can be active.
///
/// When both are set, every root span is fanned out through *both*
/// exporters. Dual-publishing (Hub + a local collector like Jaeger/Tempo)
/// is the reason this struct exists; if you want a single destination,
/// set only one env var.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedTraceEndpoints {
    /// Rerun-authed exporter routing through the Hub frontend. Set when
    /// the SDK-side `RERUN_TELEMETRY_ENDPOINT` env var is non-empty.
    ///
    /// The value is the `http(s)://` transport URL the exporter dials —
    /// the input rewritten to its underlying transport (`rerun://` and
    /// `rerun+https://` → `https://`, `rerun+http://` → `http://`) or
    /// passed through verbatim for plain `http(s)://` schemes. Any other
    /// scheme is a config error returned as `Err` by `resolve`.
    ///
    /// Never mirrored into `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` — keeping
    /// values set via the SDK-side knob out of the standard env var is
    /// the entire point of having a dedicated knob.
    rerun_authed: Option<String>,

    /// Plain OTLP gRPC exporter driven by the standard
    /// `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` (or its
    /// `OTEL_EXPORTER_OTLP_ENDPOINT` umbrella fallback). The URL is
    /// passed through verbatim — we never inspect it for `rerun://`
    /// schemes; that's the SDK-side knob's job.
    ///
    /// `Telemetry::init` mirrors this URL back into
    /// `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` so the `OTel` SDK's exporter
    /// builder reads it from there.
    standard: Option<String>,
}

impl ResolvedTraceEndpoints {
    /// Resolve which exporters (if any) to build from the two trace-endpoint
    /// inputs.
    ///
    /// * `rerun_telemetry_endpoint`: raw value of the SDK-side
    ///   `RERUN_TELEMETRY_ENDPOINT` env var (empty when unset).
    /// * `standard_otel_endpoint`: value of
    ///   `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` already merged with the
    ///   `OTEL_EXPORTER_OTLP_ENDPOINT` umbrella fallback.
    ///
    /// The two inputs are independent — both, either, or neither may
    /// produce a destination. The standard endpoint is *never* parsed for
    /// `rerun://` schemes; server-side configurations point at plain
    /// Alloy / Jaeger / Tempo collectors through this knob.
    ///
    /// Accepted schemes for `RERUN_TELEMETRY_ENDPOINT`: `rerun`,
    /// `rerun+http`, `rerun+https`, `http`, `https`. Anything else is a
    /// config error returned as `Err` even when `standard_otel_endpoint`
    /// is valid — a typo in the dedicated knob surfaces at init time
    /// instead of silently dropping the Hub destination.
    fn resolve(
        rerun_telemetry_endpoint: &str,
        standard_otel_endpoint: &str,
    ) -> anyhow::Result<Self> {
        let rerun_authed = if rerun_telemetry_endpoint.is_empty() {
            None
        } else {
            let reject = || {
                anyhow::anyhow!(
                    "RERUN_TELEMETRY_ENDPOINT={rerun_telemetry_endpoint:?} is not a supported endpoint URL — \
                     accepted schemes are rerun://, rerun+http://, rerun+https://, http://, https://"
                )
            };
            let (scheme, rest) = rerun_telemetry_endpoint
                .split_once("://")
                .ok_or_else(reject)?;
            let transport_scheme = match scheme {
                "rerun" | "rerun+https" | "https" => "https",
                "rerun+http" | "http" => "http",
                _ => return Err(reject()),
            };
            Some(format!("{transport_scheme}://{rest}"))
        };

        let standard =
            (!standard_otel_endpoint.is_empty()).then(|| standard_otel_endpoint.to_owned());

        Ok(Self {
            rerun_authed,
            standard,
        })
    }

    fn any(&self) -> bool {
        self.rerun_authed.is_some() || self.standard.is_some()
    }

    /// Short tag used in the `Telemetry initialized` log line and the
    /// init-failure stderr fallback. Keep the strings stable — operators
    /// grep these out of logs.
    fn trace_mode(&self) -> &'static str {
        match (self.rerun_authed.is_some(), self.standard.is_some()) {
            (true, true) => "rerun-authed+otlp",
            (true, false) => "rerun-authed",
            (false, true) => "otlp",
            (false, false) => "off",
        }
    }

    /// Human-readable destination(s) for the same log lines. Renders the
    /// dual-publish case as `"<rerun_url> + <std_url>"` so both URLs are
    /// visible in one grep.
    fn summary(&self) -> String {
        match (&self.rerun_authed, &self.standard) {
            (Some(rerun), Some(std)) => format!("{rerun} + {std}"),
            (Some(url), None) | (None, Some(url)) => url.clone(),
            (None, None) => "off".to_owned(),
        }
    }
}

/// `SpanExporter` decorator that refreshes the Rerun SDK auth token just-in-time
/// before each export, delegating the actual gRPC send to the inner OTLP
/// exporter.
///
/// `SpanExporter::export` is async, and per its contract is never called
/// concurrently for the same instance. Before delegating, this wrapper awaits
/// `provider.get_token()` and writes the result into the shared `token_cache`
/// that the inner exporter's synchronous tonic interceptor reads from. The
/// credentials provider has its own internal cache and short-circuits on a
/// still-valid JWT, so the steady-state cost is a single async lock read;
/// real network refresh only fires near token expiry.
///
/// On refresh failure the cache is left untouched — a stale but still-valid
/// JWT continues to be used, and the inner exporter's own error handling
/// applies if the server rejects. A single `warn!` fires on the *rising edge*
/// of a failure run, re-arming on the next success, so sustained outages
/// don't spam the log.
#[derive(Debug)]
struct AuthRefreshingSpanExporter<P: re_auth::credentials::CredentialsProvider> {
    inner: opentelemetry_otlp::SpanExporter,
    provider: Arc<P>,
    token_cache: Arc<parking_lot::RwLock<String>>,
    refresh_failing: std::sync::atomic::AtomicBool,
}

impl<P> opentelemetry_sdk::trace::SpanExporter for AuthRefreshingSpanExporter<P>
where
    P: re_auth::credentials::CredentialsProvider + Send + Sync + std::fmt::Debug + 'static,
{
    async fn export(
        &self,
        batch: Vec<opentelemetry_sdk::trace::SpanData>,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        use std::sync::atomic::Ordering;

        match self.provider.get_token().await {
            Ok(Some(jwt)) => {
                *self.token_cache.write() = jwt.to_string();
                self.refresh_failing.store(false, Ordering::Relaxed);
            }
            Ok(None) => {
                self.token_cache.write().clear();
                self.refresh_failing.store(false, Ordering::Relaxed);
            }
            Err(err) => {
                // Leave the cached token in place — if it's still inside its
                // validity window, the server will accept it.
                if !self.refresh_failing.swap(true, Ordering::Relaxed) {
                    tracing::warn!(
                        "Hub auth token refresh failed, continuing with cached token: {err}"
                    );
                }
            }
        }

        self.inner.export(batch).await
    }

    fn shutdown_with_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> opentelemetry_sdk::error::OTelSdkResult {
        self.inner.shutdown_with_timeout(timeout)
    }

    fn force_flush(&mut self) -> opentelemetry_sdk::error::OTelSdkResult {
        self.inner.force_flush()
    }

    fn set_resource(&mut self, resource: &opentelemetry_sdk::Resource) {
        self.inner.set_resource(resource);
    }
}

/// Build an OTLP `SpanExporter` that pushes through a tonic Channel whose
/// outbound requests carry a Rerun SDK Bearer token in the `authorization`
/// metadata. The token comes from
/// [`re_auth::credentials::CliCredentialsProvider`] — same global credentials
/// store the rest of the SDK uses (populated by `rerun auth login`) — and is
/// refreshed just before each export by [`AuthRefreshingSpanExporter`].
///
/// The actual TCP/TLS handshake is deferred to first use via
/// `Endpoint::connect_lazy()` so init stays sync.
fn build_rerun_authed_span_exporter(
    transport_url: &str,
) -> anyhow::Result<AuthRefreshingSpanExporter<re_auth::credentials::CliCredentialsProvider>> {
    use re_auth::credentials::CliCredentialsProvider;

    build_rerun_authed_span_exporter_with_provider(
        transport_url,
        Arc::new(CliCredentialsProvider::new()),
    )
}

/// Inner constructor parameterized on the [`re_auth::credentials::CredentialsProvider`].
/// The public [`build_rerun_authed_span_exporter`] wires up `CliCredentialsProvider`;
/// tests inject [`re_auth::credentials::StaticCredentialsProvider`] with a known JWT.
fn build_rerun_authed_span_exporter_with_provider<P>(
    transport_url: &str,
    provider: Arc<P>,
) -> anyhow::Result<AuthRefreshingSpanExporter<P>>
where
    P: re_auth::credentials::CredentialsProvider + Send + Sync + std::fmt::Debug + 'static,
{
    let token_cache: Arc<parking_lot::RwLock<String>> =
        Arc::new(parking_lot::RwLock::new(String::new()));

    // Build the tonic Channel by hand so we can attach our auth interceptor
    // and so the TLS config matches `re_redap_client` (rustls + system roots
    // via `tonic/tls-native-roots`).
    let mut endpoint: tonic::transport::Endpoint = transport_url.parse()?;
    if transport_url.starts_with("https://") {
        endpoint = endpoint.tls_config(
            tonic::transport::ClientTlsConfig::new()
                .with_enabled_roots()
                .assume_http2(true),
        )?;
    }
    let channel = endpoint.connect_lazy();

    // Single combined interceptor that both injects the Bearer token AND
    // delegates to `RerunVersionInterceptor` to set `x-rerun-client-version`.
    // Each call to `TonicExporterBuilder::with_interceptor` only accepts one
    // interceptor, so we compose them here. The standard SDK setup uses
    // `new_rerun_client_headers_layer()` but that's a tower::Layer and we'd
    // need to pass a layered service via `with_channel`, which the OTLP
    // builder doesn't allow.
    let token_for_interceptor: Arc<parking_lot::RwLock<String>> = Arc::clone(&token_cache);
    let mut version_interceptor = re_grpc_headers::RerunVersionInterceptor::new_client(None, None);
    // Rising-edge gate so a malformed cached token warns once per failure run,
    // not on every export. Mirrors `refresh_failing` on the wrapping struct.
    // Arc because the interceptor closure has to be `Clone` for `with_interceptor`.
    let parse_failing: Arc<std::sync::atomic::AtomicBool> =
        Arc::new(std::sync::atomic::AtomicBool::new(false));
    let interceptor = move |mut req: tonic::Request<()>| -> tonic::Result<tonic::Request<()>> {
        use std::sync::atomic::Ordering;
        let token = token_for_interceptor.read().clone();
        if !token.is_empty() {
            match format!("Bearer {token}").parse() {
                Ok(value) => {
                    req.metadata_mut().insert("authorization", value);
                    parse_failing.store(false, Ordering::Relaxed);
                }
                Err(err) => {
                    if !parse_failing.swap(true, Ordering::Relaxed) {
                        tracing::warn!(
                            "Cached Hub auth token failed to parse as an HTTP header value; aborting send: {err}",
                        );
                    }
                    return Err(tonic::Status::internal(
                        "cached Hub auth token is not a valid HTTP header value",
                    ));
                }
            }
        }
        tonic::service::Interceptor::call(&mut version_interceptor, req)
    };

    let inner = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_channel(channel)
        .with_interceptor(interceptor)
        .with_compression(opentelemetry_otlp::Compression::Gzip)
        .build()?;

    Ok(AuthRefreshingSpanExporter {
        inner,
        provider,
        token_cache,
        refresh_failing: std::sync::atomic::AtomicBool::new(false),
    })
}

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

/// Set to `true` by [`Telemetry::init`] once it has successfully wired up the
/// `tracing` subscriber, OTLP exporters, and global propagator. Read by
/// [`is_telemetry_active`] (and through it, by [`crate::with_tracing_session`]
/// and the Python `tracing_session()` bridge) to detect the case where a
/// caller is trying to use telemetry features before initializing the stack.
///
/// Stays `true` for the rest of the process lifetime; not cleared on
/// `Telemetry` drop (matches Python's `_is_telemetry_active` semantics).
static TELEMETRY_ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Returns `true` once [`Telemetry::init`] has run with telemetry enabled
/// (i.e. the `tracing` subscriber, OTLP exporters, and global propagator are
/// installed).
///
/// Used by [`crate::with_tracing_session`] to no-op with a warning when a
/// caller attempts session scoping before initializing telemetry. Process-
/// wide single source of truth for this question — the Python
/// `_is_telemetry_active()` binding also reads it via this function.
pub fn is_telemetry_active() -> bool {
    TELEMETRY_ACTIVE.load(std::sync::atomic::Ordering::Acquire)
}

/// Test-only: flip the [`TELEMETRY_ACTIVE`] flag without running the full
/// [`Telemetry::init`] pipeline. Lets in-crate tests exercise APIs that
/// gate on `is_telemetry_active` (notably `with_tracing_session`) without
/// having to stand up the `OTel` stack.
///
/// **Concurrency:** mutates a process-global atomic. Tests that call this
/// (or assert on `TELEMETRY_ACTIVE` / `ACTIVE_TRACING_SESSION_COUNT`) are
/// race-free only when each test runs in its own process. Use `cargo
/// nextest` (the project's standard, per `rerun/CLAUDE.md`) — it spawns
/// a subprocess per test. Plain `cargo test` runs tests as threads inside
/// one process and will be flaky against these tests.
#[cfg(test)]
pub(crate) fn set_telemetry_active_for_test(active: bool) {
    TELEMETRY_ACTIVE.store(active, std::sync::atomic::Ordering::Release);
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
    /// Same as [`Self::init`], plus registers `reader` as the host-language
    /// callback that [`crate::current_rerun_session_id`] consults on its slow
    /// path (and that [`crate::with_current_tracing_session`] invokes once at
    /// the boundary).
    ///
    /// Intended for SDK bindings (today: `rerun_py`) that hold the active
    /// session id in a host-language-specific store this crate has no way to
    /// reach. First-call-wins: the registration happens once, atomically,
    /// before `init` returns, and any subsequent registration attempt is a
    /// silent no-op.
    ///
    /// Gated behind the `session_id_reader` feature so end customers of
    /// `re_perf_telemetry` never see the extra public API.
    #[cfg(feature = "session_id_reader")]
    #[must_use = "dropping this will flush and shutdown all telemetry systems"]
    pub fn init_with_session_id_reader(
        args: TelemetryArgs,
        drop_behavior: TelemetryDropBehavior,
        reader: crate::SessionIdReader,
    ) -> anyhow::Result<Self> {
        crate::tracing_session::set_session_id_reader(reader);
        Self::init(args, drop_behavior)
    }

    #[must_use = "dropping this will flush and shutdown all telemetry systems"]
    pub fn init(args: TelemetryArgs, drop_behavior: TelemetryDropBehavior) -> anyhow::Result<Self> {
        let TelemetryArgs {
            tracy_enabled,
            enabled,
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
            metrics_listen_address: _, // TelemetryArgs only, used at the caller site
        } = args;

        // Resolve the umbrella `OTEL_EXPORTER_OTLP_ENDPOINT` as a fallback for any
        // signal-specific endpoint that wasn't set. Mirrors the OTel SDK convention.
        let umbrella_endpoint = std::env::var(OTLP_EXPORTER_ENV_VAR)
            .ok()
            .filter(|s| !s.is_empty());
        let resolve_endpoint = |signal: String| -> String {
            if !signal.is_empty() {
                signal
            } else {
                umbrella_endpoint.clone().unwrap_or_default()
            }
        };
        let log_endpoint = resolve_endpoint(log_endpoint);
        let trace_endpoint = resolve_endpoint(trace_endpoint);
        let metric_endpoint = resolve_endpoint(metric_endpoint);

        // Dedicated SDK-side trace endpoint, kept distinct from the standard
        // `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` so its value doesn't leak into
        // other OTel-aware libraries (e.g. Python's
        // `opentelemetry-exporter-otlp-proto-grpc`) sharing the same process.
        // Env-only; no clap arg, no CLI flag.
        let rerun_telemetry_endpoint =
            std::env::var("RERUN_TELEMETRY_ENDPOINT").unwrap_or_default();

        // Decide which OTLP exporters the SDK should build. See
        // [`ResolvedTraceEndpoints`] for the rules — the two endpoints are
        // independent, so it's valid for both to be active at once
        // (dual-publish to Hub and a local collector). When neither is set,
        // spans still flow through the in-process pipeline but nothing
        // leaves the process. Gated on `enabled` so a malformed
        // `RERUN_TELEMETRY_ENDPOINT` doesn't break a `TELEMETRY_ENABLED=false`
        // process (nor a `TRACY_ENABLED=true`-only one).
        let trace_endpoints = if enabled {
            ResolvedTraceEndpoints::resolve(&rerun_telemetry_endpoint, &trace_endpoint)?
        } else {
            ResolvedTraceEndpoints {
                rerun_authed: None,
                standard: None,
            }
        };

        // Pipeline summary fields. Computed once here so the success (`info!` once
        // the subscriber is up) and failure (`eprintln!`, subscriber may not be up)
        // paths can emit the same set of decision details.
        let trace_mode: &'static str = trace_endpoints.trace_mode();
        let traces_summary = trace_endpoints.summary();
        let logs_summary: String = if log_otlp_enabled && !log_endpoint.is_empty() {
            log_endpoint.clone()
        } else {
            "off".to_owned()
        };
        let metrics_summary: String = if metric_endpoint.is_empty() {
            "off".to_owned()
        } else {
            metric_endpoint.clone()
        };
        let service_name_summary: String = service_name.as_deref().unwrap_or("<unset>").to_owned();

        let result: anyhow::Result<Self> = (move || -> anyhow::Result<Self> {
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
            // Endpoint env vars are only set when we actually have an endpoint to point at;
            // overwriting them with empty strings would prevent the OTLP SDK builders from
            // reading values that may have been set externally.
            //
            // Safety: anything touching the env is unsafe, tis what it is.
            #[expect(unsafe_code)]
            unsafe {
                if !log_endpoint.is_empty() {
                    std::env::set_var("OTEL_EXPORTER_OTLP_LOGS_ENDPOINT", &log_endpoint);
                }
                if !metric_endpoint.is_empty() {
                    std::env::set_var("OTEL_EXPORTER_OTLP_METRICS_ENDPOINT", &metric_endpoint);
                }
                // Mirror the `OTEL_*`-sourced trace endpoint back into its
                // env var so the OTel SDK's exporter builder reads it from
                // there — origin/main behavior. `RERUN_TELEMETRY_ENDPOINT`
                // values live in `trace_endpoints.rerun_authed` and are
                // never mirrored here regardless of their URL scheme;
                // keeping them out of `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`
                // is the entire point of having a dedicated knob.
                if let Some(url) = &trace_endpoints.standard {
                    std::env::set_var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT", url);
                }
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
                    .add_directive_if_absent(base, "lance::execution", "warn")?
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
                    ($format:ident, $is_json:expr) => {{
                        let layer = layer
                            .$format()
                            .map_event_format(|f| TraceIdFormat::new(f, $is_json));
                        if log_test_output {
                            layer.with_test_writer().boxed()
                        } else {
                            layer.boxed()
                        }
                    }};
                }
                let layer = match log_format {
                    LogFormat::Pretty => handle_format!(pretty, false),
                    LogFormat::Compact => handle_format!(compact, false),
                    LogFormat::Json => handle_format!(json, true),
                };

                layer.with_filter(create_filter(&log_filter, "warn")?)
            };

            let (logger_provider, layer_logs_otlp) = if log_otlp_enabled && !log_endpoint.is_empty()
            {
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

            // The `TracerProvider` is always built when telemetry is enabled, so propagators
            // and `current_trace_id()` keep working. Up to two `BatchSpanProcessor`s are
            // attached — one per active trace endpoint, see [`ResolvedTraceEndpoints`].
            // With neither endpoint set, spans flow through the in-process pipeline and
            // are dropped at the end — no exporter chatter.
            let (tracer_provider, layer_traces_otlp) = {
                let mut builder = SdkTracerProvider::builder();
                if trace_endpoints.any() {
                    // Build a fresh batch config per processor — the OTel
                    // builder consumes it, and we may attach two processors
                    // when both endpoints are active.
                    let make_batch_config = || {
                        BatchConfigBuilder::default()
                            // increase max queue size from default 2048 to ensure we don't drop spans during high throughput
                            .with_max_queue_size(8192)
                            // export more spans per batch to reduce number of requests (default is 512)
                            // together with queue size this help ensure more robust exporting under high throughput
                            .with_max_export_batch_size(2048)
                            .build()
                    };

                    // Tag root spans with `rerun_session_id` whenever any
                    // exporter is active, so Tempo can find client-side
                    // traces by `{ .rerun_session_id = "rs_…" }`. When a
                    // vanilla OTLP destination is configured alongside Hub,
                    // it also receives the attribute on root spans —
                    // downstream tools that don't know about it ignore it.
                    builder = builder
                        .with_span_processor(crate::tracestate::RerunSessionRootSpanProcessor);

                    if let Some(transport_url) = &trace_endpoints.rerun_authed {
                        // `RERUN_TELEMETRY_ENDPOINT` exporter, already
                        // normalized to its `http(s)://` transport form by
                        // `ResolvedTraceEndpoints::resolve`. Injects the
                        // SDK's auth token on every export — the dedicated
                        // knob always opts into the Rerun auth path
                        // regardless of scheme.
                        let exporter = build_rerun_authed_span_exporter(transport_url)?;
                        builder = builder.with_span_processor(
                            BatchSpanProcessor::builder(exporter)
                                .with_batch_config(make_batch_config())
                                .build(),
                        );
                    }

                    if trace_endpoints.standard.is_some() {
                        // Standard OTLP exporter — reads the endpoint from
                        // `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` (mirrored
                        // above), so no explicit URL passed here.
                        let exporter = opentelemetry_otlp::SpanExporter::builder()
                            .with_tonic() // There's no good reason to use HTTP for traces (at the moment, that is)
                            .with_compression(opentelemetry_otlp::Compression::Gzip) // use gzip compression to reduce bandwidth
                            .build()?;
                        builder = builder.with_span_processor(
                            BatchSpanProcessor::builder(exporter)
                                .with_batch_config(make_batch_config())
                                .build(),
                        );
                    }
                }

                let provider = builder.build();

                // Used by `TracingInjectorInterceptor` to encode the trace information into the
                // outbound request headers. `TraceStateEnricher` runs after the W3C propagator
                // and merges `rerun_session_id=<id>` into `tracestate` whenever a tracing
                // session is active (Rust `with_tracing_session` or Python `tracing_session()`).
                // With no active scope it is a no-op.
                let propagators: Vec<
                    Box<dyn opentelemetry::propagation::TextMapPropagator + Send + Sync>,
                > = vec![
                    Box::new(opentelemetry_sdk::propagation::TraceContextPropagator::new()),
                    Box::new(crate::tracestate::TraceStateEnricher),
                ];

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
            // The `MeterProvider` is always built so the `start_metrics_listener()` Prometheus
            // path keeps working; the OTLP push exporter is only attached when an endpoint is
            // configured (per-signal or via the umbrella).
            let (metric_provider, metrics_reader) = {
                let mut builder = SdkMeterProvider::builder();

                // Use base-2 exponential histograms (OTel equivalent of Prometheus native
                // histograms) instead of explicit bucket histograms. This avoids hardcoding
                // bucket boundaries and lets the SDK auto-scale resolution.
                builder =
                    builder.with_view(|instrument: &opentelemetry_sdk::metrics::Instrument| {
                        if instrument.kind()
                            == opentelemetry_sdk::metrics::InstrumentKind::Histogram
                        {
                            opentelemetry_sdk::metrics::Stream::builder()
                                .with_aggregation(Aggregation::Base2ExponentialHistogram {
                                    // Max buckets per positive/negative range. Negative buckets
                                    // stay empty for duration/size metrics. Comparable to the
                                    // ~10 explicit buckets we had before, but with auto-scaling
                                    // boundaries.
                                    max_size: 20,
                                    // Starting resolution scale. The base of each bucket is
                                    // 2^(2^(-scale)). At scale 20 (the maximum), buckets are
                                    // extremely fine-grained; the SDK automatically downscales
                                    // when observations exceed max_size buckets.
                                    max_scale: 20,
                                    record_min_max: true,
                                })
                                .build()
                                .ok()
                        } else {
                            None
                        }
                    });

                if !metric_endpoint.is_empty() {
                    // OTLP exporter for push-based metrics
                    let otlp_exporter = opentelemetry_otlp::MetricExporter::builder()
                        .with_temporality(opentelemetry_sdk::metrics::Temporality::Cumulative)
                        .with_http()
                        .build()?;
                    builder = builder.with_periodic_exporter(otlp_exporter);
                }

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

                (Some(provider), Some(reader_for_telemetry))
            };

            if tracy_enabled {
                #[cfg(feature = "tracy")]
                {
                    tracing_subscriber::registry()
                        .with(layer_logs_otlp)
                        .with(layer_logs_and_traces_stdio)
                        .with(layer_traces_otlp)
                        .with(SpanMetadataCleanupLayer::default())
                        .with(self::tracy::tracy_layer())
                        .try_init()?;
                }

                #[cfg(not(feature = "tracy"))]
                {
                    anyhow::bail!(
                        "`TRACY_ENABLED=true` but the 'tracy' feature flag is not toggled"
                    );
                }
            } else {
                tracing_subscriber::registry()
                    .with(layer_logs_otlp)
                    .with(layer_logs_and_traces_stdio)
                    .with(layer_traces_otlp)
                    .with(SpanMetadataCleanupLayer::default())
                    .try_init()?;
            }

            crate::memory_telemetry::install_memory_use_meters();

            // Reached only on the enabled-true success path (subscriber +
            // OTLP layers installed). Flips the process-wide flag that
            // [`is_telemetry_active`] exposes; consumers like
            // [`crate::with_tracing_session`] and the Python
            // `tracing_session()` bridge gate on it.
            TELEMETRY_ACTIVE.store(true, std::sync::atomic::Ordering::Release);

            Ok(Self {
                drop_behavior,
                logs: logger_provider,
                traces: tracer_provider,
                metrics: metric_provider,
                metrics_reader,
            })
        })();

        match result {
            Ok(self_) => {
                // Emitted through the subscriber installed by `try_init` above
                // (when `enabled` or `tracy_enabled`). Drops silently in the
                // no-subscriber case — but that case has nothing else running
                // either, so silence is appropriate.
                tracing::info!(
                    enabled,
                    service = %service_name_summary,
                    trace_mode,
                    traces = %traces_summary,
                    logs = %logs_summary,
                    metrics = %metrics_summary,
                    tracy = tracy_enabled,
                    "Telemetry initialized"
                );
                #[cfg(feature = "tracy")]
                if tracy_enabled && enabled {
                    tracing::warn!(
                        "using tracy in addition to standard telemetry stack, consider `TELEMETRY_ENABLED=false`"
                    );
                }
                Ok(self_)
            }
            Err(err) => {
                // The subscriber is not guaranteed to be installed on the
                // failure path (most error sites are pre-`try_init`), so fall
                // back to stderr to ensure the diagnosis is visible.
                eprintln!(
                    "Telemetry init failed (enabled={enabled} service={service_name_summary} trace_mode={trace_mode} traces={traces_summary} logs={logs_summary} metrics={metrics_summary} tracy={tracy_enabled}): {err:#}"
                );
                Err(err)
            }
        }
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
                Ensure TELEMETRY_ENABLED=true"
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

#[cfg(test)]
mod tests {
    use super::ResolvedTraceEndpoints;

    /// Compact projection of `resolve`'s return for table-driven assertions.
    /// `Endpoints` keeps both URLs so we can match the dual-publish cases
    /// directly; `Err` collapses all malformed-input cases together — we
    /// only assert "resolve rejects this", not the particular error type.
    #[derive(Debug)]
    enum Want {
        Endpoints {
            rerun_authed: Option<&'static str>,
            standard: Option<&'static str>,
        },
        Err,
    }

    /// Shorthand constructors for the `Want::Endpoints` rows.
    const fn rerun_only(url: &'static str) -> Want {
        Want::Endpoints {
            rerun_authed: Some(url),
            standard: None,
        }
    }
    const fn standard_only(url: &'static str) -> Want {
        Want::Endpoints {
            rerun_authed: None,
            standard: Some(url),
        }
    }
    const fn both(rerun_authed: &'static str, standard: &'static str) -> Want {
        Want::Endpoints {
            rerun_authed: Some(rerun_authed),
            standard: Some(standard),
        }
    }
    const NONE: Want = Want::Endpoints {
        rerun_authed: None,
        standard: None,
    };

    /// `ResolvedTraceEndpoints::resolve` behavior, table-driven.
    ///
    /// Each row: `(rerun_telemetry_endpoint, standard_otel_endpoint, expected)`.
    #[test]
    fn resolve_behavior() {
        let cases: &[(&str, &str, Want)] = &[
            // -- No exporter ---------------------------------------------
            ("", "", NONE),
            // -- Only OTEL_*: standard, verbatim (never parsed for `rerun://`) -
            (
                "",
                "https://collector:4317",
                standard_only("https://collector:4317"),
            ),
            (
                "",
                "http://localhost:4317",
                standard_only("http://localhost:4317"),
            ),
            ("", "grpc://collector", standard_only("grpc://collector")),
            (
                "",
                "rerun://api.example.com",
                standard_only("rerun://api.example.com"),
            ),
            // -- Only RERUN_*, `rerun*` schemes: authed (normalized) -----
            (
                "rerun://api.example.com",
                "",
                rerun_only("https://api.example.com"),
            ),
            (
                "rerun+https://api.example.com:4317",
                "",
                rerun_only("https://api.example.com:4317"),
            ),
            (
                "rerun+http://localhost:4317",
                "",
                rerun_only("http://localhost:4317"),
            ),
            (
                "rerun://host/foo/bar?x=1",
                "",
                rerun_only("https://host/foo/bar?x=1"),
            ),
            // -- Only RERUN_*, plain `http(s)://`: still authed, URL unchanged ---
            (
                "https://api.example.com:4317",
                "",
                rerun_only("https://api.example.com:4317"),
            ),
            (
                "http://localhost:4317",
                "",
                rerun_only("http://localhost:4317"),
            ),
            // -- Invalid RERUN_*: Err (no silent fallback to OTEL_*) -----
            ("ftp://collector", "", Want::Err),
            ("grpc://collector", "", Want::Err),
            ("garbage", "", Want::Err),
            ("api.example.com", "", Want::Err),
            ("RERUN://host", "", Want::Err), // Case-sensitive: `re_uri::Scheme` convention.
            ("Rerun+Https://host", "", Want::Err),
            ("HTTPS://host", "", Want::Err),
            ("rerun:/host", "", Want::Err),
            ("rerun", "", Want::Err),
            ("ftp://bad", "https://otel:4317", Want::Err), // Malformed RERUN_* does NOT fall back to OTEL_*.
            // -- Both set: dual-publish (both exporters active) ----------
            (
                "rerun://hub",
                "https://collector",
                both("https://hub", "https://collector"),
            ),
            (
                "http://hub",
                "https://collector",
                both("http://hub", "https://collector"),
            ),
            (
                "rerun+http://hub:4317",
                "https://collector:4317",
                both("http://hub:4317", "https://collector:4317"),
            ),
        ];

        for (rerun, otel, want) in cases {
            let got = ResolvedTraceEndpoints::resolve(rerun, otel);
            let matches = match (&got, want) {
                (Err(_), Want::Err) => true,
                (
                    Ok(endpoints),
                    Want::Endpoints {
                        rerun_authed,
                        standard,
                    },
                ) => {
                    endpoints.rerun_authed.as_deref() == *rerun_authed
                        && endpoints.standard.as_deref() == *standard
                }
                _ => false,
            };
            assert!(
                matches,
                "resolve({rerun:?}, {otel:?})\n  got:      {got:?}\n  expected: {want:?}",
            );
        }
    }

    /// Minimal structurally-valid JWT: `{"alg":"HS256","typ":"JWT"}` base64url
    /// then `{}` then a stub signature. `re_auth::Jwt::try_from` only checks
    /// that the header decodes, so this is enough.
    const TEST_JWT: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.sig";

    #[test]
    fn build_rejects_invalid_transport_url() {
        use re_auth::credentials::StaticCredentialsProvider;

        let jwt = re_auth::Jwt::try_from(TEST_JWT.to_owned()).unwrap();
        let provider = super::Arc::new(StaticCredentialsProvider::new(jwt));
        let result =
            super::build_rerun_authed_span_exporter_with_provider("not a url at all", provider);
        assert!(result.is_err(), "expected Err for malformed URL");
    }

    /// End-to-end: build the authed exporter with a known JWT, send a span
    /// through it, and confirm the `MockOtlpCollector` receives an export
    /// whose `authorization` metadata is `Bearer <jwt>`. Exercises the
    /// full wrapper → tonic interceptor → gRPC metadata pipeline.
    #[tokio::test(flavor = "multi_thread")]
    async fn authed_exporter_sends_bearer_metadata() {
        use std::time::Duration;

        use opentelemetry::trace::{Tracer as _, TracerProvider as _};
        use opentelemetry_sdk::trace::{BatchSpanProcessor, SdkTracerProvider};
        use re_auth::credentials::StaticCredentialsProvider;
        use re_test_mocks::otlp::MockOtlpCollector;

        let collector = MockOtlpCollector::spawn().await;
        let jwt = re_auth::Jwt::try_from(TEST_JWT.to_owned()).unwrap();
        let provider = super::Arc::new(StaticCredentialsProvider::new(jwt));

        let exporter =
            super::build_rerun_authed_span_exporter_with_provider(&collector.endpoint(), provider)
                .unwrap();

        let tracer_provider = SdkTracerProvider::builder()
            .with_span_processor(BatchSpanProcessor::builder(exporter).build())
            .build();
        let tracer = tracer_provider.tracer("test");

        // Emit one span, then force a flush so we don't wait for the default
        // 5-second scheduled-delay tick.
        {
            let span = tracer.start("authed_test_span");
            drop(span);
        }
        tracer_provider.force_flush().ok();

        let received = collector
            .wait_for(|_| true, Duration::from_secs(10))
            .await
            .expect("collector should receive at least one span");

        let auth = received
            .metadata
            .get("authorization")
            .expect("authorization metadata missing")
            .to_str()
            .expect("authorization should be ASCII");
        assert_eq!(auth, format!("Bearer {TEST_JWT}"));
    }
}
