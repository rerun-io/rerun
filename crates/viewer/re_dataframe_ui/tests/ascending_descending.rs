mod common;

use std::sync::Arc;

use arrow::array::{RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use datafusion::prelude::SessionContext;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use re_dataframe_ui::{DataFusionTableWidget, SortBy, TableBlueprint};
use re_test_context::TestContext;
use re_viewer_context::AsyncRuntimeHandle;

use common::run_async_harness;

#[tokio::test]
async fn test_no_sort() {
    let (session_context, table_ref) = prepare_session_context();
    let test_context = TestContext::new();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([600.0, 400.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(Arc::clone(&session_context), table_ref)
                    .title("No sort")
                    .show(ctx, &runtime_handle, ui);
            });
        });

    run_async_harness(&mut harness).await;
    harness.snapshot("test_no_sort");
}

#[tokio::test]
async fn test_ascending() {
    let (session_context, table_ref) = prepare_session_context();
    let test_context = TestContext::new();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([600.0, 400.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(Arc::clone(&session_context), table_ref)
                    .title("Ascending")
                    .initial_blueprint(TableBlueprint {
                        sort_by: Some(SortBy::ascending("col")),
                        ..Default::default()
                    })
                    .show(ctx, &runtime_handle, ui);
            });
        });

    run_async_harness(&mut harness).await;
    harness.snapshot("test_ascending");
}

#[tokio::test]
async fn test_descending() {
    let (session_context, table_ref) = prepare_session_context();
    let test_context = TestContext::new();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([600.0, 400.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(Arc::clone(&session_context), table_ref)
                    .title("Descending")
                    .initial_blueprint(TableBlueprint {
                        sort_by: Some(SortBy::descending("col")),
                        ..Default::default()
                    })
                    .show(ctx, &runtime_handle, ui);
            });
        });

    run_async_harness(&mut harness).await;
    harness.snapshot("test_descending");
}

#[tokio::test]
async fn test_column_menu_button() {
    let (session_context, table_ref) = prepare_session_context();
    let test_context = TestContext::new();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([600.0, 400.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(Arc::clone(&session_context), table_ref)
                    .title("Column menu button")
                    .show(ctx, &runtime_handle, ui);
            });
        });

    run_async_harness(&mut harness).await;
    let node = harness
        .query_all_by_role_and_label(Role::Button, "More options")
        .next()
        .unwrap();
    node.click();
    run_async_harness(&mut harness).await;
    harness.snapshot("test_column_menu_button");
}

// ---

fn prepare_session_context() -> (Arc<SessionContext>, &'static str) {
    // create a record batch with a single string column
    let schema = Arc::new(Schema::new_with_metadata(
        vec![Field::new("col", DataType::Utf8, false)],
        Default::default(),
    ));
    let batch = RecordBatch::try_new_with_options(
        schema.clone(),
        vec![Arc::new(StringArray::from(vec!["b", "a", "c"]))],
        &Default::default(),
    )
    .expect("Failed to create a record batch");

    // create a datafusion session context with that table
    let session_context = Arc::new(SessionContext::new());
    let table_ref = "test_table";
    session_context
        .register_batch(table_ref, batch)
        .expect("Failed to register the table");

    (session_context, table_ref)
}
