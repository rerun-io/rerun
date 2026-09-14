mod common;

use std::sync::Arc;

use arrow::array::{Array as _, ListArray, StringBuilder};
use arrow::datatypes::{Field, Schema};
use datafusion::prelude::SessionContext;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use re_async::AsyncRuntimeHandle;
use re_dataframe_ui::{DataFusionTableWidget, SortBy, TableBlueprints};
use re_test_context::TestContext;
use re_viewer_context::TableReference;

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
                DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    table_ref,
                    TableReference::local("test_table"),
                )
                .title("No sort")
                .show(
                    ctx.app_ctx,
                    &runtime_handle,
                    ui,
                    &TableBlueprints::default(),
                    &mut test_context.view_states.lock(),
                );
            });
        });

    run_async_harness(&test_context, &mut harness).await;
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
                DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    table_ref,
                    TableReference::local("test_table"),
                )
                .title("Ascending")
                .sort_by(SortBy::ascending("col".into()))
                .show(
                    ctx.app_ctx,
                    &runtime_handle,
                    ui,
                    &TableBlueprints::default(),
                    &mut test_context.view_states.lock(),
                );
            });
        });

    run_async_harness(&test_context, &mut harness).await;
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
                DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    table_ref,
                    TableReference::local("test_table"),
                )
                .title("Descending")
                .sort_by(SortBy::descending("col".into()))
                .show(
                    ctx.app_ctx,
                    &runtime_handle,
                    ui,
                    &TableBlueprints::default(),
                    &mut test_context.view_states.lock(),
                );
            });
        });

    run_async_harness(&test_context, &mut harness).await;
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
                DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    table_ref,
                    TableReference::local("test_table"),
                )
                .title("Column menu button")
                .show(
                    ctx.app_ctx,
                    &runtime_handle,
                    ui,
                    &TableBlueprints::default(),
                    &mut test_context.view_states.lock(),
                );
            });
        });

    run_async_harness(&test_context, &mut harness).await;
    let node = harness
        .query_all_by_role_and_label(Role::Button, "More options")
        .next()
        .unwrap();
    node.click();
    run_async_harness(&test_context, &mut harness).await;
    harness.snapshot("test_column_menu_button");
}

// ---

fn prepare_session_context() -> (Arc<SessionContext>, &'static str) {
    // create a record batch with a single string list column
    let column = ListArray::from_nested_iter::<StringBuilder, _, _, _>(vec![
        Some(vec![Some("b")]),
        None,
        Some(vec![Some("a")]),
        Some(vec![None]),
        Some(vec![]),
        Some(vec![Some("c")]),
    ]);

    let schema = Arc::new(Schema::new_with_metadata(
        vec![Field::new("col", column.data_type().clone(), true)],
        Default::default(),
    ));
    common::register_test_table("test_table", schema, vec![Arc::new(column)])
}
