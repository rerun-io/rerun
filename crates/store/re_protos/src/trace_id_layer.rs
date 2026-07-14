use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use opentelemetry::TraceId;
use tower::Service;
use tower::layer::Layer;

/// The HTTP header key for the request trace ID, used to correlate responses with
/// distributed traces for debugging and support.
pub const RERUN_HTTP_HEADER_REQUEST_TRACE_ID: &str = "x-request-trace-id";

/// A function that returns the current trace ID, if any.
pub type TraceIdProvider = Arc<dyn Fn() -> Option<TraceId> + Send + Sync>;

/// A [`tower::Layer`] that injects a trace ID into all responses
/// via the [`RERUN_HTTP_HEADER_REQUEST_TRACE_ID`] header.
///
/// The trace ID is obtained by calling the provided [`TraceIdProvider`].
///
/// See [`TraceIdService`].
#[derive(Clone)]
pub struct TraceIdLayer {
    trace_id_provider: TraceIdProvider,
}

impl TraceIdLayer {
    pub fn new(trace_id_provider: TraceIdProvider) -> Self {
        Self { trace_id_provider }
    }
}

impl<S> Layer<S> for TraceIdLayer {
    type Service = TraceIdService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TraceIdService {
            inner,
            trace_id_provider: Arc::clone(&self.trace_id_provider),
        }
    }
}

/// A [`tower::Service`] that injects a trace ID into all responses
/// via the [`RERUN_HTTP_HEADER_REQUEST_TRACE_ID`] header.
///
/// See [`TraceIdLayer`].
#[derive(Clone)]
pub struct TraceIdService<S> {
    inner: S,
    trace_id_provider: TraceIdProvider,
}

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for TraceIdService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        // See: https://docs.rs/tower/latest/tower/trait.Service.html#be-careful-when-cloning-inner-services
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let trace_id_provider = Arc::clone(&self.trace_id_provider);

        Box::pin(async move {
            let mut response = inner.call(req).await?;

            if let Some(trace_id) = (trace_id_provider)() {
                let trace_id = trace_id.to_string();
                match http::HeaderValue::from_str(&trace_id) {
                    Ok(header_value) => {
                        response
                            .headers_mut()
                            .insert(RERUN_HTTP_HEADER_REQUEST_TRACE_ID, header_value);
                    }
                    Err(err) => {
                        tracing::warn!(
                            trace_id,
                            %err,
                            "failed to convert trace ID to header value"
                        );
                    }
                }
            }

            Ok(response)
        })
    }
}
