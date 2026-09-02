use std::sync::Arc;
use std::time::{Duration, Instant};

use arrow::array::ArrayRef;
use arrow::datatypes::SchemaRef;
use arrow::record_batch::RecordBatch;
use datafusion::prelude::SessionContext;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use re_test_context::TestContext;

/// Register an in-memory record batch as a DataFusion test table.
pub fn register_test_table(
    table_ref: &'static str,
    schema: SchemaRef,
    columns: Vec<ArrayRef>,
) -> (Arc<SessionContext>, &'static str) {
    let batch = RecordBatch::try_new_with_options(schema, columns, &Default::default())
        .expect("test record batch should be valid");
    let session_context = Arc::new(SessionContext::new());
    session_context
        .register_batch(table_ref, batch)
        .expect("test table should register");
    (session_context, table_ref)
}

/// Run the harness until no loading indicators are present.
///
/// Polls the harness and yields to tokio between steps so datafusion can make progress.
pub async fn run_async_harness<State>(
    test_context: &TestContext,
    harness: &mut Harness<'_, State>,
) {
    // generous timeout to avoid flakiness
    let timeout = Duration::from_secs(20);
    let start = Instant::now();
    loop {
        assert!(
            start.elapsed() <= timeout,
            "Test timed out waiting for table to load"
        );

        harness.run_steps(1);
        test_context.handle_system_commands(&harness.ctx);
        harness.run_steps(1);

        // let datafusion do some work!
        tokio::task::yield_now().await;

        if harness.query_by_role(Role::ProgressIndicator).is_none() {
            break;
        }
    }
}
