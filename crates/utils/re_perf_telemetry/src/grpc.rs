// --- Telemetry middlewares ---

/// Implements [`tower_http::trace::MakeSpan`] where the trace name is the gRPC method name.
///
/// We keep track of a bunch of relevant in-house state associated with the span in `SpanMetadata`.
#[derive(Clone)]
pub struct GrpcMakeSpan {
    gauge: opentelemetry::metrics::Gauge<u64>,
}

impl GrpcMakeSpan {
    #[expect(clippy::new_without_default)] // future-proofing
    pub fn new() -> Self {
        let meter = opentelemetry::global::meter("grpc");
        let gauge = meter
            .u64_gauge("grpc_make_span_state_size")
            .with_description("Size of the SpanMetadata state")
            .build();
        Self { gauge }
    }
}

impl<B> tower_http::trace::MakeSpan<B> for GrpcMakeSpan {
    fn make_span(&mut self, request: &http::Request<B>) -> tracing::Span {
        let endpoint = request.uri().path().to_owned();
        let url = request
            .uri()
            .to_string()
            .strip_suffix(&endpoint)
            .map(ToOwned::to_owned);

        let email = request
            .headers()
            .get("authorization")
            .and_then(|auth| auth.to_str().ok().and_then(|s| s.strip_prefix("Bearer ")))
            .and_then(|token| token.split('.').skip(1).take(1).next())
            .and_then(|data| {
                use base64::{Engine as _, engine::general_purpose};
                general_purpose::STANDARD_NO_PAD.decode(data).ok()
            })
            .and_then(|data| {
                #[derive(serde::Deserialize)]
                struct TokenData {
                    sub: String,
                }
                serde_json::from_slice::<TokenData>(&data)
                    .ok()
                    .map(|data| data.sub)
            });

        let dataset_id = request
            .headers()
            .get("x-rerun-dataset-id")
            .and_then(|v| v.to_str().ok().map(ToOwned::to_owned));

        // NOTE: Remember: the span we're creating here will propagate no matter what -- there is
        // no sampling at the `tracing` level, only at the `opentelemetry` level.
        // We use that fact to our advantage in order to carry a bunch of state around across all
        // the stages of the request (first response, first chunk, end-of-stream, etc).
        let mut safe_headers = request.headers().clone();
        _ = safe_headers.remove("authorization");
        let span = tracing::span!(
            tracing::Level::INFO,
            "<request>",
            otel.name = %endpoint,
            url,
            method = %request.method(),
            version = ?request.version(),
            headers = ?safe_headers,
            email,
            dataset_id,
        );

        let size = SpanMetadata::insert_opt(
            span.id(),
            SpanMetadata {
                endpoint,
                email,
                dataset_id,
                first_chunk_returned: false,
                grpc_eos_classifier: None,
            },
            false,
        );
        self.gauge.record(size as _, &[]);

        span
    }
}

/// The global storage for [`SpanMetadata`]s.
///
/// Keeps track of relevant in-house context/metadata for all on-going gRPC spans.
///
/// We could also build a full-fledged `tracing::Subscriber` instead, but at this point I'd rather
/// _accomplish something_ instead of implementing yet another 50 layers of abstraction.
///
/// The state is written to and read from by our different gRPC middlewares. In particular,
/// [`GrpcOnEos`] is responsible for cleaning up dead entries.
static SPAN_METADATA: std::sync::OnceLock<
    parking_lot::RwLock<ahash::HashMap<tracing::span::Id, SpanMetadata>>,
> = std::sync::OnceLock::new();

/// Custom state/context/metadata that we associate with the spans we generate in our [`GrpcMakeSpan`] middleware.
///
/// All this state is stored in `SPAN_METADATA`.
#[derive(Debug, Clone)]
struct SpanMetadata {
    /// Which gRPC endpoint? Extracted from h2 headers.
    endpoint: String,

    /// What email, if any? Extracted from h2 auth headers.
    email: Option<String>,

    /// What dataset ID, if any? Extracted from h2 Rerun extension headers.
    dataset_id: Option<String>,

    /// Has the gRPC stream associated with this span streamed back its first chunk of data yet?
    ///
    /// This is set by our [`GrpcOnFirstBodyChunk`] middleware.
    first_chunk_returned: bool,

    /// If the gRPC stream's failure outcome is to be determined by its response stream, this will
    /// tell us how.
    ///
    /// This is set by our [`GrpcOnResponse`] middleware.
    grpc_eos_classifier: Option<tower_http::classify::GrpcEosErrorsAsFailures>,
}

impl Default for SpanMetadata {
    fn default() -> Self {
        Self {
            endpoint: "undefined".to_owned(),
            email: None,
            dataset_id: None,
            first_chunk_returned: false,
            grpc_eos_classifier: None,
        }
    }
}

impl SpanMetadata {
    /// Returns the new size of the map.
    #[expect(clippy::needless_pass_by_value)]
    fn insert(span_id: tracing::span::Id, metadata: Self, expect_conflict: bool) -> usize {
        let (is_overwrite, new_len) = {
            let mut state = SPAN_METADATA.get_or_init(Default::default).write();
            let is_overwrite = state.insert(span_id.clone(), metadata).is_some();
            let new_len = state.len();
            (is_overwrite, new_len)
        };

        if is_overwrite && !expect_conflict {
            tracing::warn!(id=?span_id, "overwritten span metadata -- this should never happen");
        }

        new_len
    }

    /// Returns the new size of the map.
    fn insert_opt(
        span_id: Option<tracing::span::Id>,
        metadata: Self,
        expect_conflict: bool,
    ) -> usize {
        if let Some(span_id) = span_id {
            Self::insert(span_id, metadata, expect_conflict)
        } else {
            SPAN_METADATA.get_or_init(Default::default).read().len()
        }
    }

    fn get(span_id: &tracing::span::Id) -> Option<Self> {
        let md = SPAN_METADATA
            .get()
            .and_then(|spans| spans.read().get(span_id).cloned());

        if md.is_none() {
            tracing::warn!(id=?span_id, "missing span metadata -- this should never happen");
        }

        md
    }

    fn get_opt(span_id: Option<&tracing::span::Id>) -> Option<Self> {
        span_id.and_then(Self::get)
    }

    fn remove(span_id: &tracing::span::Id) -> Option<Self> {
        let md = SPAN_METADATA
            .get()
            .and_then(|spans| spans.write().remove(span_id));

        if md.is_none() {
            tracing::warn!(id=?span_id, "missing span metadata -- this should never happen");
        }

        md
    }

    fn remove_opt(span_id: Option<&tracing::span::Id>) -> Option<Self> {
        span_id.and_then(Self::remove)
    }
}

// ---

/// Implements a [`tower_http::trace::OnRequest`] middleware.
#[derive(Clone)]
pub struct GrpcOnRequest {}

impl GrpcOnRequest {
    #[expect(clippy::new_without_default)] // future-proofing
    pub fn new() -> Self {
        Self {}
    }
}

impl<B> tower_http::trace::OnRequest<B> for GrpcOnRequest {
    fn on_request(&mut self, _request: &http::Request<B>, _span: &tracing::Span) {
        tracing::trace!("grpc_on_request");
    }
}

// ---

/// Implements a [`tower_http::trace::OnResponse`] middleware.
#[derive(Clone)]
pub struct GrpcOnResponse {
    histogram: opentelemetry::metrics::Histogram<f64>,
}

impl GrpcOnResponse {
    #[expect(clippy::new_without_default)] // future-proofing
    pub fn new() -> Self {
        let meter = opentelemetry::global::meter("grpc");
        let histogram = meter
            .f64_histogram("grpc_on_response_ms")
            .with_description("Latency percentiles for all gRPC endpoints (\"time to response\")")
            .with_boundaries(vec![
                10.0, 25.0, 50.0, 75.0, 100.0, 200.0, 350.0, 500.0, 750.0, 1000.0, 2500.0, 5000.0,
            ])
            .build();
        Self { histogram }
    }
}

impl<B> tower_http::trace::OnResponse<B> for GrpcOnResponse {
    fn on_response(
        self,
        response: &http::Response<B>,
        latency: std::time::Duration,
        span: &tracing::Span,
    ) {
        let Some(span_metadata) = SpanMetadata::get_opt(span.id().as_ref()) else {
            return;
        };

        let SpanMetadata {
            endpoint,
            email,
            dataset_id,
            first_chunk_returned: _,
            grpc_eos_classifier: _,
        } = span_metadata.clone();

        let record = |grpc_code: tonic::Code| {
            let grpc_status = format!("{grpc_code:?}"); // NOTE: The debug repr is the enum variant name (e.g. DeadlineExceeded).
            let http_status = response.status().as_str().to_owned();

            let email = email.as_deref().unwrap_or("undefined");
            let dataset_id = dataset_id.as_deref().unwrap_or("undefined");

            // NOTE: repeat all these attributes so services such as CloudWatch, which don't really
            // support OTLP, can actually see them.
            if grpc_status == "Ok" {
                tracing::info!(%endpoint, %grpc_status, %http_status, %email, %dataset_id, ?latency, "grpc_on_response");
            } else {
                tracing::error!(%endpoint, %grpc_status, %http_status, %email, %dataset_id, ?latency, "grpc_on_response");
            }

            self.histogram.record(
                latency.as_secs_f64() * 1000.0,
                &[
                    opentelemetry::KeyValue::new("endpoint", endpoint),
                    opentelemetry::KeyValue::new("grpc_status", grpc_status),
                    opentelemetry::KeyValue::new("http_status", http_status),
                    opentelemetry::KeyValue::new("email", email.to_owned()),
                    opentelemetry::KeyValue::new("dataset_id", dataset_id.to_owned()),
                ],
            );
        };

        use tower_http::classify::ClassifyResponse as _;
        let classified =
            tower_http::classify::GrpcErrorsAsFailures::new().classify_response(response);
        match classified {
            tower_http::classify::ClassifiedResponse::Ready(Err(err)) => match err {
                tower_http::classify::GrpcFailureClass::Code(code) => {
                    record(tonic::Code::from_i32(code.into()));
                }

                tower_http::classify::GrpcFailureClass::Error(err) => {
                    record(tonic::Status::from_error(err.into()).code());
                }
            },

            tower_http::classify::ClassifiedResponse::Ready(Ok(_)) => {
                record(tonic::Code::Ok);
            }

            tower_http::classify::ClassifiedResponse::RequiresEos(eos) => {
                record(tonic::Code::Ok);
                SpanMetadata::insert_opt(
                    span.id(),
                    SpanMetadata {
                        grpc_eos_classifier: Some(eos),
                        ..span_metadata
                    },
                    true,
                );
            }
        }
    }
}

/// Implements a [`tower_http::trace::OnBodyChunk`] middleware, but only accounts for the first one.
#[derive(Clone)]
pub struct GrpcOnFirstBodyChunk {
    histogram: opentelemetry::metrics::Histogram<f64>,
}

impl GrpcOnFirstBodyChunk {
    #[expect(clippy::new_without_default)] // future-proofing
    pub fn new() -> Self {
        let meter = opentelemetry::global::meter("grpc");
        let histogram = meter
            .f64_histogram("grpc_on_first_body_chunk_ms")
            .with_description(
                "Latency percentiles for all gRPC endpoints (\"time to first chunk\")",
            )
            .with_boundaries(vec![
                10.0, 25.0, 50.0, 75.0, 100.0, 200.0, 350.0, 500.0, 750.0, 1000.0, 2500.0, 5000.0,
            ])
            .build();
        Self { histogram }
    }
}

impl<B> tower_http::trace::OnBodyChunk<B> for GrpcOnFirstBodyChunk {
    fn on_body_chunk(&mut self, _: &B, latency: std::time::Duration, span: &tracing::Span) {
        let Some(span_metadata) = SpanMetadata::get_opt(span.id().as_ref()) else {
            return;
        };

        let SpanMetadata {
            endpoint,
            email,
            dataset_id,
            first_chunk_returned,
            grpc_eos_classifier: _,
        } = span_metadata.clone();

        if !first_chunk_returned {
            let email = email.as_deref().unwrap_or("undefined");
            let dataset_id = dataset_id.as_deref().unwrap_or("undefined");

            // NOTE: repeat all these attributes so services such as CloudWatch, which don't really
            // support OTLP, can actually see them.
            tracing::debug!(%endpoint, %email, %dataset_id, ?latency, "grpc_on_first_body_chunk");

            self.histogram.record(
                latency.as_secs_f64() * 1000.0,
                &[
                    opentelemetry::KeyValue::new("endpoint", endpoint),
                    opentelemetry::KeyValue::new("email", email.to_owned()),
                    opentelemetry::KeyValue::new("dataset_id", dataset_id.to_owned()),
                ],
            );

            SpanMetadata::insert_opt(
                span.id(),
                SpanMetadata {
                    first_chunk_returned: true,
                    ..span_metadata
                },
                true,
            );
        }
    }
}

/// Implements a [`tower_http::trace::OnEos`] middleware.
///
/// Note that even unary endpoints are implemented as streams internally, and will therefore be
/// impacted by this middleware. This is especially important at this middleware is responsible for
/// GC'ing the contents of `SPAN_METADATA`.
#[derive(Clone)]
pub struct GrpcOnEos {
    counter: opentelemetry::metrics::Counter<u64>,
}

impl GrpcOnEos {
    #[expect(clippy::new_without_default)] // future-proofing
    pub fn new() -> Self {
        let meter = opentelemetry::global::meter("grpc");
        let counter = meter
            .u64_counter("grpc_on_eos")
            .with_description("End-of-stream counter for all gRPC endpoints")
            .build();
        Self { counter }
    }
}

impl tower_http::trace::OnEos for GrpcOnEos {
    fn on_eos(
        self,
        trailers: Option<&http::HeaderMap>,
        duration: std::time::Duration,
        span: &tracing::Span,
    ) {
        let Some(span_metadata) = SpanMetadata::remove_opt(span.id().as_ref()) else {
            return;
        };

        let SpanMetadata {
            endpoint,
            email,
            dataset_id,
            first_chunk_returned: _,
            grpc_eos_classifier,
        } = span_metadata;

        let grpc_code = if let Some(classifier) = grpc_eos_classifier {
            use tower_http::classify::ClassifyEos as _;
            match classifier.classify_eos(trailers) {
                Ok(()) => tonic::Code::Ok,
                Err(err) => match err {
                    tower_http::classify::GrpcFailureClass::Code(code) => {
                        tonic::Code::from_i32(code.into())
                    }
                    tower_http::classify::GrpcFailureClass::Error(err) => {
                        tonic::Status::from_error(err.into()).code()
                    }
                },
            }
        } else {
            tracing::warn!(
                endpoint,
                email,
                dataset_id,
                "couldn't determine gRPC EOS status code"
            );
            tonic::Code::Unknown
        };
        let grpc_status = format!("{grpc_code:?}"); // NOTE: The debug repr is the enum variant name (e.g. DeadlineExceeded).

        let email = email.as_deref().unwrap_or("undefined");
        let dataset_id = dataset_id.as_deref().unwrap_or("undefined");

        // NOTE: repeat all these attributes so services such as CloudWatch, which don't really
        // support OTLP, can actually see them.
        if grpc_status == "Ok" {
            tracing::info!(%endpoint, %grpc_status, %email, %dataset_id, ?duration, "grpc_on_eos");
        } else {
            tracing::error!(%endpoint, %grpc_status, %email, %dataset_id, ?duration, "grpc_on_eos");
        }

        self.counter.add(
            1,
            &[
                opentelemetry::KeyValue::new("endpoint", endpoint),
                opentelemetry::KeyValue::new("grpc_status", grpc_status),
                opentelemetry::KeyValue::new("email", email.to_owned()),
                opentelemetry::KeyValue::new("dataset_id", dataset_id.to_owned()),
            ],
        );
    }
}

pub type ServerTelemetryLayer = tower::layer::util::Stack<
    tonic::service::interceptor::InterceptorLayer<TracingExtractorInterceptor>,
    tower::layer::util::Stack<
        tower_http::trace::TraceLayer<
            tower_http::trace::GrpcMakeClassifier,
            GrpcMakeSpan,
            GrpcOnRequest,
            GrpcOnResponse,
            GrpcOnFirstBodyChunk,
            GrpcOnEos,
        >,
        tower::layer::util::Stack<
            tower_http::propagate_header::PropagateHeaderLayer,
            tower::layer::util::Stack<
                tower_http::propagate_header::PropagateHeaderLayer,
                tower::layer::util::Identity,
            >,
        >,
    >,
>;

/// Creates a new [`tower::Layer`] middleware that automatically:
/// * Traces gRPC requests and responses.
/// * Logs all gRPC responses (status, latency, etc).
/// * Measures all gRPC responses (status, latency, etc).
pub fn new_server_telemetry_layer() -> ServerTelemetryLayer {
    use tower_http::propagate_header::PropagateHeaderLayer;
    let dataset_id_propagation_layer =
        PropagateHeaderLayer::new(http::HeaderName::from_static("x-rerun-dataset-id"));
    let request_id_propagation_layer =
        PropagateHeaderLayer::new(http::HeaderName::from_static("x-request-id"));

    let trace_layer = tower_http::trace::TraceLayer::new_for_grpc()
        .make_span_with(GrpcMakeSpan::new())
        .on_request(GrpcOnRequest::new())
        .on_response(GrpcOnResponse::new())
        .on_body_chunk(GrpcOnFirstBodyChunk::new())
        .on_eos(GrpcOnEos::new());

    tower::ServiceBuilder::new()
        .layer(dataset_id_propagation_layer)
        .layer(request_id_propagation_layer)
        .layer(trace_layer)
        .layer(TracingExtractorInterceptor::new_layer())
        .into_inner()
}

pub type ClientTelemetryLayer = tower::layer::util::Stack<
    tonic::service::interceptor::InterceptorLayer<TracingInjectorInterceptor>,
    tower::layer::util::Stack<
        tower_http::trace::TraceLayer<tower_http::trace::GrpcMakeClassifier, GrpcMakeSpan>,
        tower::layer::util::Stack<
            tower_http::propagate_header::PropagateHeaderLayer,
            tower::layer::util::Stack<
                tower_http::propagate_header::PropagateHeaderLayer,
                tower::layer::util::Identity,
            >,
        >,
    >,
>;

/// Creates a new [`tower::Layer`] middleware that automatically:
/// * Traces gRPC requests and responses.
/// * Logs all gRPC responses (status, latency, etc).
/// * Measures all gRPC responses (status, latency, etc).
//
// TODO(cmc): at the moment there's little value to have anything beyond traces on the client, but
// we ultimately can add all the same things that we have on the server as we need them.
pub fn new_client_telemetry_layer() -> ClientTelemetryLayer {
    use tower_http::propagate_header::PropagateHeaderLayer;
    let dataset_id_propagation_layer =
        PropagateHeaderLayer::new(http::HeaderName::from_static("x-rerun-dataset-id"));
    let request_id_propagation_layer =
        PropagateHeaderLayer::new(http::HeaderName::from_static("x-request-id"));

    let trace_layer =
        tower_http::trace::TraceLayer::new_for_grpc().make_span_with(GrpcMakeSpan::new());

    tower::ServiceBuilder::new()
        .layer(dataset_id_propagation_layer)
        .layer(request_id_propagation_layer)
        .layer(trace_layer)
        .layer(TracingInjectorInterceptor::new_layer())
        .into_inner()
}

// --- Propagation middlewares ---

/// This implements a [`tonic::service::Interceptor`] that injects trace/span metadata into the
/// request headers, according to W3C standards.
///
/// This trace/span information is extracted from the currently opened [`tracing::Span`], then
/// converting to the `OpenTelemetry` format, and finally injected into the request headers, thereby
/// propagating the trace across network boundaries.
///
/// See also [`TracingExtractorInterceptor`].
#[derive(Default, Clone)]
pub struct TracingInjectorInterceptor;

impl TracingInjectorInterceptor {
    /// Creates a new [`tower::Layer`] middleware that automatically applies the injector.
    ///
    /// See also [`new_client_telemetry_layer`].
    pub fn new_layer() -> tonic::service::interceptor::InterceptorLayer<Self> {
        tonic::service::interceptor::InterceptorLayer::new(Self)
    }
}

impl tonic::service::Interceptor for TracingInjectorInterceptor {
    fn call(&mut self, mut req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        struct MetadataMap<'a>(&'a mut tonic::metadata::MetadataMap);

        impl opentelemetry::propagation::Injector for MetadataMap<'_> {
            fn set(&mut self, key: &str, value: String) {
                if let Ok(key) = tonic::metadata::MetadataKey::from_bytes(key.as_bytes()) {
                    if let Ok(val) = tonic::metadata::MetadataValue::try_from(&value) {
                        self.0.insert(key, val);
                    }
                }
            }
        }

        // Grab the trace information from `tracing`, and convert that into `opentelemetry`.
        use tracing_opentelemetry::OpenTelemetrySpanExt as _;
        let cx = tracing::Span::current().context();

        // Inject the opentelemetry-formatted trace information into the headers.
        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&cx, &mut MetadataMap(req.metadata_mut()));
        });

        Ok(req)
    }
}

/// This implements a [`tonic::service::Interceptor`] that extracts trace/span metadata from the
/// request headers, according to W3C standards.
///
/// This trace/span information (which is still an `OpenTelemetry` payload, at that point) is then
/// injected back into the currently opened [`tracing::Span`] (if any), therefore propagating the
/// trace across network boundaries.
#[derive(Default, Clone)]
pub struct TracingExtractorInterceptor;

impl TracingExtractorInterceptor {
    /// Creates a new [`tower::Layer`] middleware that automatically applies the extractor.
    ///
    /// See also [`new_server_telemetry_layer`].
    pub fn new_layer() -> tonic::service::interceptor::InterceptorLayer<Self> {
        tonic::service::interceptor::InterceptorLayer::new(Self)
    }
}

impl tonic::service::Interceptor for TracingExtractorInterceptor {
    fn call(&mut self, req: tonic::Request<()>) -> Result<tonic::Request<()>, tonic::Status> {
        struct MetadataMap<'a>(&'a tonic::metadata::MetadataMap);

        impl opentelemetry::propagation::Extractor for MetadataMap<'_> {
            fn get(&self, key: &str) -> Option<&str> {
                self.0.get(key).and_then(|metadata| metadata.to_str().ok())
            }

            fn keys(&self) -> Vec<&str> {
                self.0
                    .keys()
                    .map(|key| match key {
                        tonic::metadata::KeyRef::Ascii(v) => v.as_str(),
                        tonic::metadata::KeyRef::Binary(v) => v.as_str(),
                    })
                    .collect::<Vec<_>>()
            }
        }

        // Grab the trace information from the headers, in OpenTelemetry format.
        let parent_ctx = opentelemetry::global::get_text_map_propagator(|prop| {
            prop.extract(&MetadataMap(req.metadata()))
        });

        // Convert the trace information back into `tracing` and inject it into the current span (if any).
        use tracing_opentelemetry::OpenTelemetrySpanExt as _;
        tracing::Span::current().set_parent(parent_ctx);

        Ok(req)
    }
}
