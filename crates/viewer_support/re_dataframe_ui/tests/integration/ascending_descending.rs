use crate::common;

use std::sync::Arc;

use arrow::array::{Array as _, ListArray, StringBuilder};
use arrow::datatypes::{Field, Schema};
use datafusion::prelude::SessionContext;
use egui::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use re_async::AsyncRuntimeHandle;
use re_dataframe_ui::{DataFusionTableWidget, SortBy, TableBlueprints};
use re_sdk_types::blueprint::archetypes::TableBlueprint;
use re_sdk_types::blueprint::components::SortBy as BlueprintSortBy;
use re_test_context::TestContext;
use re_viewer_context::TableReference;

use common::run_async_harness;

#[tokio::test]
async fn test_no_sort() {
    let test_context = TestContext::new();
    let table_blueprints = TableBlueprints::default();
    let mut harness = table_harness(&test_context, &table_blueprints, |widget| {
        widget.title("No sort")
    });

    run_async_harness(&test_context, &mut harness).await;
    harness.snapshot("test_no_sort");
}

#[tokio::test]
async fn test_ascending() {
    let test_context = TestContext::new();
    let table_blueprints = TableBlueprints::default();
    let mut harness = table_harness(&test_context, &table_blueprints, |widget| {
        widget
            .title("Ascending")
            .default_sort_by(SortBy::ascending("col".into()))
    });

    run_async_harness(&test_context, &mut harness).await;
    harness.snapshot("test_ascending");
}

#[tokio::test]
async fn test_descending() {
    let test_context = TestContext::new();
    let table_blueprints = TableBlueprints::default();
    let mut harness = table_harness(&test_context, &table_blueprints, |widget| {
        widget
            .title("Descending")
            .default_sort_by(SortBy::descending("col".into()))
    });

    run_async_harness(&test_context, &mut harness).await;
    harness.snapshot("test_descending");
}

#[tokio::test]
async fn test_column_menu_button() {
    let test_context = TestContext::new();
    let table_blueprints = TableBlueprints::default();
    let mut harness = table_harness(&test_context, &table_blueprints, |widget| {
        widget.title("Column menu button")
    });

    run_async_harness(&test_context, &mut harness).await;
    open_column_menu(&test_context, &mut harness).await;
    harness.snapshot("test_column_menu_button");
}

/// Picking a sort direction in the column menu writes the sort to the table blueprint.
#[tokio::test]
async fn test_sort_from_menu_saves_to_blueprint() {
    let test_context = TestContext::new();
    let table_blueprints =
        common::setup_table_blueprint(&test_context, &TableBlueprint::update_fields());
    let mut harness = table_harness(&test_context, &table_blueprints, |widget| widget);

    run_async_harness(&test_context, &mut harness).await;
    assert_eq!(blueprint_sort(&test_context, &table_blueprints), None);

    open_column_menu(&test_context, &mut harness).await;
    harness.get_by_label("Descending").click();
    run_async_harness(&test_context, &mut harness).await;

    assert_eq!(
        blueprint_sort(&test_context, &table_blueprints),
        Some(SortBy::descending("col".into()))
    );
}

/// A sort in the table blueprint takes precedence over the default sort of the widget. The column
/// menu disables the direction the table is already sorted in.
#[tokio::test]
async fn test_blueprint_sort_overrides_default_sort() {
    let test_context = TestContext::new();
    let table_blueprints = common::setup_table_blueprint(
        &test_context,
        &TableBlueprint::update_fields()
            .with_sort_by(BlueprintSortBy::from(&SortBy::descending("col".into()))),
    );
    let mut harness = table_harness(&test_context, &table_blueprints, |widget| {
        widget.default_sort_by(SortBy::ascending("col".into()))
    });

    run_async_harness(&test_context, &mut harness).await;
    open_column_menu(&test_context, &mut harness).await;

    assert!(is_disabled(&harness, "Descending"));
    assert!(!is_disabled(&harness, "Ascending"));
}

/// A sort in the table blueprint on a column the table doesn't have shows as a pill in the
/// filter bar. Removing the pill clears the sort from the table blueprint.
#[tokio::test]
async fn test_missing_sort_column() {
    let test_context = TestContext::new();
    let missing_sort = SortBy::ascending("missing".into());
    let table_blueprints = common::setup_table_blueprint(
        &test_context,
        &TableBlueprint::update_fields().with_sort_by(BlueprintSortBy::from(&missing_sort)),
    );
    let mut harness = table_harness(&test_context, &table_blueprints, |widget| {
        widget.title("Missing sort column")
    });

    run_async_harness(&test_context, &mut harness).await;
    harness.get_by_label(r#"Can't sort by "missing""#);
    harness.snapshot("test_missing_sort_column");

    harness.get_by_label("Remove sort").click();
    run_async_harness(&test_context, &mut harness).await;

    assert_eq!(blueprint_sort(&test_context, &table_blueprints), None);
    assert!(harness.query_by_label("Remove sort").is_none());
}

// ---

/// Shows the test table with the given table blueprints, after `configure` sets up the widget.
fn table_harness<'a>(
    test_context: &'a TestContext,
    table_blueprints: &'a TableBlueprints,
    configure: impl Fn(DataFusionTableWidget<'static>) -> DataFusionTableWidget<'static> + 'a,
) -> Harness<'a> {
    let (session_context, table_ref) = prepare_session_context();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()
        .expect("test should run inside its Tokio runtime");

    test_context
        .setup_kittest_for_rendering_ui([600.0, 400.0])
        .build_ui(move |ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                configure(DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    table_ref,
                    TableReference::local("test_table"),
                ))
                .show(
                    ctx.app_ctx,
                    &runtime_handle,
                    ui,
                    table_blueprints,
                    &mut test_context.view_states.lock(),
                );
            });
        })
}

/// Opens the menu of the first column.
async fn open_column_menu(test_context: &TestContext, harness: &mut Harness<'_>) {
    harness
        .query_all_by_role_and_label(Role::Button, "More options")
        .next()
        .expect("table should have a column menu button")
        .click();
    run_async_harness(test_context, harness).await;
}

fn is_disabled(harness: &Harness<'_>, label: &str) -> bool {
    harness.get_by_label(label).accesskit_node().is_disabled()
}

/// The sort in the active table blueprint.
fn blueprint_sort(
    test_context: &TestContext,
    table_blueprints: &TableBlueprints,
) -> Option<SortBy> {
    common::read_active_table_blueprint::<BlueprintSortBy>(
        test_context,
        table_blueprints,
        TableBlueprint::descriptor_sort_by().component,
    )?
    .into_iter()
    .next()
    .map(SortBy::from)
}

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
