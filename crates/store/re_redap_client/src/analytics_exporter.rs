//! Analytics-side OTLP exporter for a given [`crate::grpc::RedapClientInner`].
//!
//! `re_datafusion` constructs OTLP `ExportTraceServiceRequest`s for query
//! analytics and hands them to [`ConnectionAnalyticsExporter`]. The exporter
//! reuses the same authenticated tower service as REDAP RPCs, so analytics
//! exports share the HTTP/2 connection to the Hub.

use opentelemetry_proto::tonic::collector::trace::v1::{
    ExportTraceServiceRequest, ExportTraceServiceResponse,
};
use re_protos::trace_id_layer::RERUN_HTTP_HEADER_REQUEST_TRACE_ID;
use re_uri::Origin;

const EXPORT_PATH: &str = "/opentelemetry.proto.collector.trace.v1.TraceService/Export";

/// Analytics-side capability handed to `re_datafusion`.
///
/// Wraps a clone of the layered tower service shared with
/// [`crate::ConnectionClient`] — same auth / version / propagate-headers
/// stack — without requiring downstream crates to expose [`crate::RedapClientInner`].
#[derive(Clone, Debug)]
pub struct ConnectionAnalyticsExporter {
    origin: Origin,
    service: crate::grpc::RedapClientInner,
}

impl ConnectionAnalyticsExporter {
    pub(crate) fn from_remote_service(
        origin: Origin,
        service: crate::grpc::RedapClientInner,
    ) -> Self {
        Self { origin, service }
    }

    /// Origin this exporter is connected to.
    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    /// Send a single OTLP `ExportTraceServiceRequest` to the Hub.
    ///
    /// Client-side gzip is intentionally not enabled here: a viewer running a
    /// newer SDK could otherwise send `grpc-encoding: gzip` to a Hub whose
    /// `OtelIngestService` predates `accept_compressed(Gzip)`, causing analytics
    /// events to be silently dropped with an `UNIMPLEMENTED` rejection.
    // TODO(andrea): Enable gzip support when Hub 0.14.0 or newer is deployed on all stacks.
    pub async fn export_trace(
        &self,
        request: ExportTraceServiceRequest,
        trace_id: Option<opentelemetry::TraceId>,
    ) -> tonic::Result<()> {
        let mut grpc = tonic::client::Grpc::new(self.service.clone());

        let mut request = tonic::Request::new(request);
        if let Some(trace_id) = trace_id
            && let Ok(value) = trace_id.to_string().parse()
        {
            request
                .metadata_mut()
                .insert(RERUN_HTTP_HEADER_REQUEST_TRACE_ID, value);
        }

        grpc.ready().await.map_err(|err| {
            tonic::Status::unavailable(format!("analytics channel not ready: {err}"))
        })?;

        let path = http::uri::PathAndQuery::from_static(EXPORT_PATH);
        let codec = tonic_prost::ProstCodec::default();

        let _response: tonic::Response<ExportTraceServiceResponse> =
            grpc.unary(request, path, codec).await?;

        Ok(())
    }
}
