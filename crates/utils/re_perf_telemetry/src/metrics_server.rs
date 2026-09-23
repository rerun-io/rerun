//! HTTP server for metrics collection and exposition

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse as _, Response};
use axum::routing::get;
use opentelemetry_sdk::metrics::ManualReader;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::metrics::reader::MetricReader as _;
use parking_lot::Mutex;
use tokio::net::TcpListener;
use tracing::error;

use crate::prometheus::{
    MetricContainer, convert_to_prometheus, encode_registry, encode_registry_protobuf,
};

/// Start a metrics server that binds synchronously and serves asynchronously.
///
/// Returns the bound socket address after successful binding.
/// The server continues running in the spawned task.
pub(crate) async fn start_metrics_server(
    address: &str,
    reader: Arc<ManualReader>,
) -> anyhow::Result<SocketAddr> {
    let addr: SocketAddr = address.parse().map_err(|err| {
        anyhow::anyhow!("Failed to parse metrics listen address '{address}': {err}")
    })?;

    let app = Router::new()
        .route("/metrics", get(manual_metrics_handler))
        .with_state(reader);

    // Bind synchronously to catch binding errors immediately
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|err| anyhow::anyhow!("Failed to bind to {addr}: {err}"))?;

    let bound_addr = listener
        .local_addr()
        .map_err(|err| anyhow::anyhow!("Failed to get local address: {err}"))?;

    // Spawn the server task to run asynchronously
    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, app).await {
            error!("Metrics server error: {err}");
        }
    });

    tracing::info!("Metrics server started on http://{bound_addr}/metrics");

    Ok(bound_addr)
}

/// Content type of the Prometheus protobuf exposition, served when a scraper negotiates it.
const PROTOBUF_CONTENT_TYPE: &str =
    "application/vnd.google.protobuf; proto=io.prometheus.client.MetricFamily; encoding=delimited";

const TEXT_CONTENT_TYPE: &str = "text/plain; version=0.0.4";

/// Handler for the ManualReader-based /metrics endpoint.
///
/// This collects metrics on-demand from `OpenTelemetry's` `ManualReader`.
///
/// Exposing metrics is meant to be cheap in every moment.
/// As long as determining the data to be exposed is not cheap it has to be somewhat cached and made available cheaply.
///
/// The response format is negotiated from `Accept`: protobuf when the client asks for it,
/// text otherwise.
async fn manual_metrics_handler(
    headers: HeaderMap,
    State(reader): State<Arc<ManualReader>>,
) -> Response {
    // This handler is picking up data from telemetry SDK's ManualReader,
    // this is a temporary solution to expose metrics in different ways
    // (pull and push).
    // This is to be replaced in the future with a less complex solution,
    // using only a single approach.
    let mut resource_metrics = ResourceMetrics::default();

    if let Err(err) = reader.collect(&mut resource_metrics) {
        error!("Failed to collect metrics from ManualReader: {err}");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain")],
            format!("Failed to collect metrics: {err}"),
        )
            .into_response();
    }

    let metrics = Arc::new(Mutex::new(MetricContainer::new()));
    let registry = convert_to_prometheus(&resource_metrics, &metrics);

    let encoded = if wants_protobuf(&headers) {
        encode_registry_protobuf(&registry)
            .map(|buffer| (PROTOBUF_CONTENT_TYPE, buffer))
            .map_err(|err| err.to_string())
    } else {
        encode_registry(&registry)
            .map(|text| (TEXT_CONTENT_TYPE, text.into_bytes()))
            .map_err(|err| err.to_string())
    };

    match encoded {
        Ok((content_type, body)) => {
            (StatusCode::OK, [(header::CONTENT_TYPE, content_type)], body).into_response()
        }
        Err(err) => {
            error!("Failed to encode metrics: {err}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain")],
                format!("Failed to encode metrics: {err}"),
            )
                .into_response()
        }
    }
}

/// Whether the client can parse the Prometheus protobuf exposition.
///
/// Matching the family is enough: Prometheus and Alloy only advertise it when they want
/// native histograms. Removing `PrometheusProto` turns it off, reordering does not.
fn wants_protobuf(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::ACCEPT)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|accept| accept.split(','))
        .any(|entry| {
            entry.contains("proto=io.prometheus.client.MetricFamily")
                && entry.contains("encoding=delimited")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accept(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::ACCEPT, value.parse().unwrap());
        headers
    }

    /// The accepted header is the one Prometheus and Alloy send for native histograms.
    #[test]
    fn protobuf_is_served_only_for_the_prometheus_family() {
        assert!(wants_protobuf(&accept(
            "application/vnd.google.protobuf;proto=io.prometheus.client.MetricFamily;\
             encoding=delimited;q=0.9,application/openmetrics-text;version=1.0.0;q=0.8,\
             text/plain;version=0.0.4;q=0.5,*/*;q=0.1"
        )));

        for header in [
            "*/*",
            "text/plain",
            // The `OpenMetrics` protobuf: same media type, different payload.
            "application/vnd.google.protobuf;proto=io.openmetrics.MetricSet;encoding=delimited",
            // Our family, but an encoding we do not produce.
            "application/vnd.google.protobuf;proto=io.prometheus.client.MetricFamily;encoding=text",
        ] {
            assert!(!wants_protobuf(&accept(header)), "{header}");
        }

        assert!(!wants_protobuf(&HeaderMap::new()));
    }
}
