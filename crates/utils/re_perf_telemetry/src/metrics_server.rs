//! HTTP server for metrics collection and exposition

use parking_lot::Mutex;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Router,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use opentelemetry_sdk::metrics::{ManualReader, data::ResourceMetrics, reader::MetricReader as _};
use tokio::net::TcpListener;
use tracing::error;

use crate::prometheus::{MetricContainer, convert_to_prometheus, encode_registry};

/// Start a metrics server that binds synchronously and serves asynchronously.
/// Returns the bound socket address after successful binding.
/// The server continues running in the spawned task.
pub(crate) async fn start_metrics_server(
    address: &str,
    reader: Arc<ManualReader>,
) -> anyhow::Result<SocketAddr> {
    let addr: SocketAddr = address.parse().map_err(|err| {
        anyhow::anyhow!(
            "Failed to parse metrics listen address '{}': {}",
            address,
            err
        )
    })?;

    let app = Router::new()
        .route("/metrics", get(manual_metrics_handler))
        .with_state(reader);

    // Bind synchronously to catch binding errors immediately
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|err| anyhow::anyhow!("Failed to bind to {}: {}", addr, err))?;

    let bound_addr = listener
        .local_addr()
        .map_err(|err| anyhow::anyhow!("Failed to get local address: {}", err))?;

    // Spawn the server task to run asynchronously
    tokio::spawn(async move {
        if let Err(err) = axum::serve(listener, app).await {
            error!("Metrics server error: {}", err);
        }
    });

    Ok(bound_addr)
}

/// Handler for the ManualReader-based /metrics endpoint
/// This collects metrics on-demand from `OpenTelemetry's` `ManualReader`
async fn manual_metrics_handler(State(reader): State<Arc<ManualReader>>) -> impl IntoResponse {
    // This handler is picking up data from telemetry SDK's ManualReader,
    // this is a temporary solution to expose metrics in different ways
    // (pull and push).
    // This is to be replaced in the future with a less complex solution,
    // using only a single approach.
    let mut resource_metrics = ResourceMetrics::default();

    // Collect metrics from ManualReader
    match reader.collect(&mut resource_metrics) {
        Ok(_) => {
            let metrics = Arc::new(Mutex::new(MetricContainer::new()));

            // Convert ResourceMetrics to Prometheus metrics and get the registry
            let registry = convert_to_prometheus(&resource_metrics, &metrics);

            // Encode metrics to Prometheus text format
            match encode_registry(&registry) {
                Ok(buffer) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
                    buffer,
                ),
                Err(err) => {
                    error!("Failed to encode metrics: {}", err);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(header::CONTENT_TYPE, "text/plain")],
                        format!("Failed to encode metrics: {err}"),
                    )
                }
            }
        }
        Err(err) => {
            error!("Failed to collect metrics from ManualReader: {}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain")],
                format!("Failed to collect metrics: {err}"),
            )
        }
    }
}
