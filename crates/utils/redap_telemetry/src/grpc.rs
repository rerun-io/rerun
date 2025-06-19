// --- Telemetry ---

/// Implements [`tower_http::trace::MakeSpan`] where the trace name is the gRPC method name.
#[derive(Debug, Default, Clone, Copy)]
pub struct GrpcSpanMaker;

impl<B> tower_http::trace::MakeSpan<B> for GrpcSpanMaker {
    fn make_span(&mut self, request: &http::Request<B>) -> tracing::Span {
        tracing::span!(
            tracing::Level::INFO,
            "<ignored>",
            otel.name = %request.uri().path(),
            method = %request.method(),
            uri = %request.uri(),
            version = ?request.version(),
            headers = ?request.headers(),
        )
    }
}

/// Creates a new [`tower::Layer`] middleware that automatically traces gRPC requests and responses.
///
/// Works for both clients and servers.
pub fn new_grpc_tracing_layer()
-> tower_http::trace::TraceLayer<tower_http::trace::GrpcMakeClassifier, GrpcSpanMaker> {
    tower_http::trace::TraceLayer::new_for_grpc().make_span_with(GrpcSpanMaker)
}

// --- Propagation ---

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
    /// See also [`new_grpc_tracing_layer`].
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
    /// See also [`new_grpc_tracing_layer`].
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
