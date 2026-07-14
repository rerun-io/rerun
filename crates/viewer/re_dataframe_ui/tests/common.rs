use std::time::{Duration, Instant};

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

/// Run the harness until no loading indicators are present.
///
/// Polls the harness and yields to tokio between steps so datafusion can make progress.
pub async fn run_async_harness<State>(harness: &mut Harness<'_, State>) {
    // generous timeout to avoid flakiness
    let timeout = Duration::from_secs(20);
    let start = Instant::now();
    loop {
        assert!(
            start.elapsed() <= timeout,
            "Test timed out waiting for table to load"
        );

        harness.run_steps(2);

        // let datafusion do some work!
        tokio::task::yield_now().await;

        if harness.query_by_role(Role::ProgressIndicator).is_none() {
            break;
        }
    }
}
