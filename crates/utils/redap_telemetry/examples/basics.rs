//! Example of using our telemetry tools.
//!
//! Usage:
//! * Start the telemetry stack: `pixi run compose`
//! * Run this example: `cargo r --example basics`
//! * Go to <http://localhost:16686/search> to explore the logs and traces.
//! * Go to <http://localhost:9090/query> to explore the metrics.
//!   * Check out <http://localhost:9090/api/v1/label/__name__/values> to list all available metrics.
//!   * Try e.g. this query: `sum(is_even_histogram_bucket) by (le)`

use tracing::{Instrument as _, instrument};

// ---

#[instrument(err)]
async fn is_even(i: i32) -> anyhow::Result<()> {
    simulate_latency().await;
    anyhow::ensure!(i % 2 == 0, "oh no, `i` is odd!!");
    Ok(())
}

#[tokio::main]
async fn main() {
    // Safety: anything touching the env is unsafe, tis what it is.
    #[expect(unsafe_code)]
    unsafe {
        std::env::set_var("OTEL_SERVICE_NAME", "redap-telemetry-example");
    }

    use clap::Parser as _;
    // Take a look at `TelemetryArgs` to learn more about all the configurable things.
    let args = redap_telemetry::TelemetryArgs::parse_from(std::env::args());

    // This is the complete telemetry pipeline. Everything will be flushed when this gets dropped.
    let _telemetry =
        redap_telemetry::Telemetry::init(args, redap_telemetry::TelemetryDropBehavior::Shutdown);

    let scope = opentelemetry::InstrumentationScope::builder("redap-telemetry-example").build();
    let metrics = opentelemetry::global::meter_with_scope(scope);

    let is_even_histogram = metrics
        .f64_histogram("is_even_histogram")
        .with_description("Latency percentiles for `is_even`")
        .with_boundaries(vec![
            10.0, 20.0, 30.0, 40.0, 60.0, 80.0, 100.0, 200.0, 400.0, 1000.0,
        ])
        .build();

    for batch in [0..20, 20..40, 40..60] {
        let span = tracing::info_span!("main_loop", ?batch);
        async {
            for i in batch.clone() {
                let now = tokio::time::Instant::now();

                if let Err(err) = is_even(i).await {
                    tracing::error!(%err, i, "not even!");
                } else {
                    tracing::info!(i, "is even!");
                }

                is_even_histogram.record(
                    now.elapsed().as_millis() as _,
                    &[opentelemetry::KeyValue::new("batch", format!("{batch:?}"))],
                );
            }
        }
        .instrument(span) // instrumenting async scopes is tricky!
        .await;
    }
}

// ---

async fn simulate_latency() {
    use rand::Rng as _;
    let p: u16 = rand::thread_rng().gen_range(1..=1000);

    // p70: 10ms
    // p80: 15ms
    // p90: 30ms
    // p95: 50ms
    // p99: 70ms
    // p999: 150ms
    let delay_ms = match p {
        1..=700 => 10,
        701..=800 => 15,
        801..=900 => 30,
        901..=950 => 50,
        951..=990 => 70,
        _ => 150,
    };

    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
}
