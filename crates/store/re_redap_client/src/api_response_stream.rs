use tokio_stream::{Stream, StreamExt as _};

use crate::{ApiError, ApiResult, extract_trace_id};

/// A stream that optionally carries a server-assigned trace-id
/// from the initial gRPC response metadata.
///
/// Functions consuming the stream should attach the trace-id to any errors they produce,
/// and pass it along to any [`ApiResponseStream`] they return.
pub struct ApiResponseStream<T> {
    inner: std::pin::Pin<Box<dyn Stream<Item = ApiResult<T>> + Send>>,
    trace_id: Option<opentelemetry::TraceId>,
}

impl<T> ApiResponseStream<T> {
    pub fn new(
        inner: impl Stream<Item = ApiResult<T>> + Send + 'static,
        trace_id: Option<opentelemetry::TraceId>,
    ) -> Self {
        Self {
            inner: Box::pin(inner),
            trace_id,
        }
    }

    pub fn trace_id(&self) -> Option<opentelemetry::TraceId> {
        self.trace_id
    }
}

impl<T> Stream for ApiResponseStream<T> {
    type Item = ApiResult<T>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

impl<T: Send + 'static> ApiResponseStream<T> {
    /// Creates an [`ApiResponseStream`] from a streaming [`tonic::Response`],
    /// extracting the trace-id from the response metadata and converting
    /// tonic stream errors to [`ApiError`]s.
    pub fn from_tonic_response(
        response: tonic::Response<tonic::Streaming<T>>,
        endpoint: &'static str,
    ) -> Self {
        let trace_id = extract_trace_id(response.metadata());
        let stream = response.into_inner().map(move |item| {
            item.map_err(|err| {
                // Warn-log transport-level stream failures with the gRPC code so that
                // connection teardowns (Cancelled/Unavailable from HTTP/2 keep-alive
                // timeout, peer GOAWAY, etc.) are visible client-side instead of just
                // being mapped silently into `ApiError`. We log here — not at every
                // call-site — because this is the single funnel for streaming RPCs.
                tracing::warn!(
                    endpoint,
                    grpc_code = %err.code(),
                    error = %err,
                    trace_id = trace_id.map(|t| t.to_string()).as_deref(),
                    "gRPC streaming response failed"
                );
                ApiError::tonic(err, format!("{endpoint} stream failed")).with_trace_id(trace_id)
            })
        });
        Self {
            inner: Box::pin(stream),
            trace_id,
        }
    }
}
