//! HTTP server for metrics collection and exposition

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse as _, Response};
use axum::routing::get;
use opentelemetry_sdk::metrics::ManualReader;
use opentelemetry_sdk::metrics::data::ResourceMetrics;
use opentelemetry_sdk::metrics::reader::MetricReader as _;
use parking_lot::Mutex;
use prometheus_client::encoding::prometheus_protobuf;
use tokio::net::TcpListener;
use tracing::error;

use crate::prometheus::{MetricContainer, convert_to_prometheus};

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

const PROTOBUF_CONTENT_TYPE: &str =
    "application/vnd.google.protobuf; proto=io.prometheus.client.MetricFamily; encoding=delimited";

/// Handler for the ManualReader-based /metrics endpoint.
///
/// This collects metrics on-demand from `OpenTelemetry's` `ManualReader`.
///
/// Exposing metrics is meant to be cheap in every moment.
/// As long as determining the data to be exposed is not cheap it has to be somewhat cached and made available cheaply.
///
/// Serves the Prometheus protobuf exposition only; scrapers must list `PrometheusProto`.
async fn manual_metrics_handler(State(reader): State<Arc<ManualReader>>) -> Response {
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

    match prometheus_protobuf::encode_to_vec(&registry) {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, PROTOBUF_CONTENT_TYPE)],
            body,
        )
            .into_response(),
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
