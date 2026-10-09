//! Test the filter bar of the `DataFusionTableWidget`.

use crate::common;

use std::sync::Arc;

use arrow::array::{Int64Array, TimestampNanosecondArray};
use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use datafusion::prelude::SessionContext;
use egui::{Key, Modifiers, Role};
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use egui_kittest::{Harness, SnapshotResults};
use re_async::AsyncRuntimeHandle;
use re_chunk_store::external::re_chunk::Chunk;
use re_dataframe_ui::{ColumnFilter, DataFusionTableWidget, TableBlueprints, TimestampFilter};
use re_entity_db::EntityDb;
use re_log_types::{StoreId, StoreKind};
use re_sdk_types::blueprint::archetypes::TableBlueprint;
use re_sdk_types::blueprint::components::SqlFilterExpression;
use re_test_context::TestContext;
use re_viewer_context::{TableReference, blueprint_timepoint_for_writes};

use common::run_async_harness;

/// A filter added from the column menu stays in the filter bar while its popup is open, even if
/// the blueprint filters change. Escape discards it, and the filter bar shows the blueprint
/// filters again.
#[tokio::test(flavor = "multi_thread")] // `multi_thread` required because `ConnectionRegistryHandle::credentials` uses `block_in_place`.
async fn test_filter_edit_survives_blueprint_change() {
    let (session_context, table_ref) = setup_test_table();
    let test_context = TestContext::new();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();
    let table_blueprints = setup_table_blueprint(&test_context, &[r#""a" > 1"#.to_owned()]);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([800.0, 400.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    table_ref,
                    TableReference::local("test_table"),
                )
                .show(
                    ctx.app_ctx,
                    &runtime_handle,
                    ui,
                    &table_blueprints,
                    &mut test_context.view_states.lock(),
                );
            });
        });

    run_async_harness(&test_context, &mut harness).await;
    assert_eq!(filter_pill_labels(&harness), ["a > 1"]);

    // Add a filter on the first column from its menu.
    harness
        .query_all_by_role_and_label(Role::Button, "More options")
        .next()
        .unwrap()
        .click();
    run_async_harness(&test_context, &mut harness).await;
    harness.get_by_label("Filter").click();
    run_async_harness(&test_context, &mut harness).await;
    assert_eq!(filter_pill_labels(&harness), ["a > 1", "a == …"]);

    let changed_filters = vec![
        r#""a" > 1"#.to_owned(),
        r#""a" < "a""#.to_owned(),
        r#""a" < 5"#.to_owned(),
    ];
    write_active_filters(&test_context, &table_blueprints, &changed_filters);
    run_async_harness(&test_context, &mut harness).await;
    assert_eq!(filter_pill_labels(&harness), ["a > 1", "a == …"]);

    harness.key_press(Key::Escape);
    run_async_harness(&test_context, &mut harness).await;
    assert_eq!(
        filter_pill_labels(&harness),
        ["a > 1", r#"Custom "a" filter"#, "a < 5"]
    );
    assert_eq!(
        blueprint_filters(&test_context, &table_blueprints),
        changed_filters
    );
}

/// Committing a typed timestamp writes the filter to the blueprint, and the reopened popup shows
/// the timestamp in its canonical form.
#[tokio::test(flavor = "multi_thread")] // `multi_thread` required because `ConnectionRegistryHandle::credentials` uses `block_in_place`.
async fn test_timestamp_filter_on_commit() {
    let (session_context, table_ref) = setup_test_table();
    let test_context = TestContext::new();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();
    let table_blueprints = setup_table_blueprint(&test_context, &[timestamp_filter_sql()]);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([800.0, 400.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    table_ref,
                    TableReference::local("test_table"),
                )
                .show(
                    ctx.app_ctx,
                    &runtime_handle,
                    ui,
                    &table_blueprints,
                    &mut test_context.view_states.lock(),
                );
            });
        });

    run_async_harness(&test_context, &mut harness).await;
    harness
        .get_by_label("ts is after 1973-03-03 09:46:40Z")
        .click();
    run_async_harness(&test_context, &mut harness).await;

    // Select the text of the timestamp input.
    harness.get_by_role(Role::TextInput).click();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    run_async_harness(&test_context, &mut harness).await;

    harness.get_by_role(Role::TextInput).type_text("1979-07-10");
    harness.key_press(Key::Enter);
    run_async_harness(&test_context, &mut harness).await;
    assert_eq!(
        blueprint_filters(&test_context, &table_blueprints),
        [r#""ts" >= TIMESTAMP '1979-07-10T00:00:00Z'"#]
    );

    harness
        .get_by_label("ts is after 1979-07-10 00:00:00Z")
        .click();
    run_async_harness(&test_context, &mut harness).await;
    assert_eq!(
        harness.get_by_role(Role::TextInput).value().as_deref(),
        Some("1979-07-10 00:00:00Z")
    );

    harness.snapshot_options(
        "timestamp_filter_on_commit",
        &re_ui::testing::default_snapshot_options_for_ui(),
    );
}

/// A filter that only fails once the query runs shows the table error together with the filter
/// bar, which still marks filters that failed to plan. Removing the failing filter from the
/// filter bar loads the table again.
#[tokio::test(flavor = "multi_thread")] // `multi_thread` required because `ConnectionRegistryHandle::credentials` uses `block_in_place`.
async fn test_remove_filter_after_query_error() {
    let (session_context, table_ref) = setup_test_table();
    let test_context = TestContext::new();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();
    let valid_filter = r#""a" > 1"#.to_owned();
    let unknown_column_filter = r#""missing" > 1"#.to_owned();
    let table_blueprints = setup_table_blueprint(
        &test_context,
        &[
            valid_filter.clone(),
            unknown_column_filter.clone(),
            r#"CAST(CONCAT(CAST("a" AS VARCHAR), 'x') AS BIGINT) > 1"#.to_owned(),
        ],
    );

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([800.0, 400.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    table_ref,
                    TableReference::local("test_table"),
                )
                .title("Test table")
                .show(
                    ctx.app_ctx,
                    &runtime_handle,
                    ui,
                    &table_blueprints,
                    &mut test_context.view_states.lock(),
                );
            });
        });

    run_async_harness(&test_context, &mut harness).await;
    assert!(
        harness
            .query_by_label_contains("Could not load table")
            .is_some()
    );
    assert_eq!(
        filter_pill_labels(&harness),
        [
            "a > 1",
            r#"Custom "missing" filter"#,
            r#"Custom "a" filter"#
        ]
    );
    harness.snapshot_options(
        "remove_filter_after_query_error_1_error",
        &re_ui::testing::default_snapshot_options_for_ui(),
    );

    harness
        .query_all_by_label("Remove filter")
        .nth(2)
        .unwrap()
        .click();
    run_async_harness(&test_context, &mut harness).await;
    assert_eq!(
        blueprint_filters(&test_context, &table_blueprints),
        [valid_filter, unknown_column_filter]
    );
    assert!(
        harness
            .query_by_label_contains("Could not load table")
            .is_none()
    );
    assert_eq!(
        filter_pill_labels(&harness),
        ["a > 1", r#"Custom "missing" filter"#]
    );
    harness.snapshot_options(
        "remove_filter_after_query_error_2_removed",
        &re_ui::testing::default_snapshot_options_for_ui(),
    );
}

/// A failing filter shows as an error pill next to the valid filters, and the table still loads
/// with the valid filters. SQL that doesn't parse gets a plain error pill, and SQL that fails to
/// plan gets a pill named after its column. Hovering the error pill shows its error.
#[tokio::test(flavor = "multi_thread")] // `multi_thread` required because `ConnectionRegistryHandle::credentials` uses `block_in_place`.
async fn test_filter_planning_errors() {
    struct ErrorCase {
        name: &'static str,
        sql: &'static str,
        pill_label: &'static str,
    }

    let cases = [
        ErrorCase {
            name: "parse_error",
            sql: r#"(("a" > 1)"#,
            pill_label: "Error",
        },
        ErrorCase {
            name: "unknown_column",
            sql: r#""missing" > 1"#,
            pill_label: r#"Custom "missing" filter"#,
        },
        ErrorCase {
            name: "wrong_function_arguments",
            sql: r#"abs("a", 1)"#,
            pill_label: r#"Custom "a" filter"#,
        },
    ];

    let mut snapshot_results = SnapshotResults::new();
    for case in cases {
        let (session_context, table_ref) = setup_test_table();
        let test_context = TestContext::new();
        let runtime_handle =
            AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen().unwrap();
        let table_blueprints = setup_table_blueprint(
            &test_context,
            &[r#""a" > 1"#.to_owned(), case.sql.to_owned()],
        );

        let mut harness = test_context
            .setup_kittest_for_rendering_ui([600.0, 300.0])
            .build_ui(|ui| {
                test_context.run_recording(&ui.ctx().clone(), |ctx| {
                    DataFusionTableWidget::new(
                        Arc::clone(&session_context),
                        table_ref,
                        TableReference::local("test_table"),
                    )
                    .title("Test table")
                    .show(
                        ctx.app_ctx,
                        &runtime_handle,
                        ui,
                        &table_blueprints,
                        &mut test_context.view_states.lock(),
                    );
                });
            });

        run_async_harness(&test_context, &mut harness).await;
        assert!(
            harness
                .query_by_label_contains("Could not load table")
                .is_none(),
            "{}",
            case.name
        );
        assert_eq!(
            filter_pill_labels(&harness),
            ["a > 1", case.pill_label],
            "{}",
            case.name
        );

        harness.get_by_label(case.pill_label).hover();
        harness.try_run_realtime().ok();

        snapshot_results.add(harness.try_snapshot_options(
            format!("filter_planning_error_{}", case.name),
            &re_ui::testing::default_snapshot_options_for_ui(),
        ));
    }
}

// ---

/// The labels of the pills in the filter bar.
fn filter_pill_labels<State>(harness: &Harness<'_, State>) -> Vec<String> {
    harness
        .query_all_by_role(Role::Unknown)
        .filter_map(|node| node.accesskit_node().label())
        .collect()
}

fn timestamp_filter_sql() -> String {
    ColumnFilter::new(
        Arc::new(timestamp_field()),
        TimestampFilter::after(
            jiff::Timestamp::from_millisecond(100_000_000_000).expect("timestamp is in range"),
        ),
    )
    .to_sql()
    .expect("filter selects some rows")
}

fn timestamp_field() -> Field {
    Field::new(
        "ts",
        DataType::Timestamp(TimeUnit::Nanosecond, Some("UTC".into())),
        false,
    )
}

/// Sets up a default table blueprint with the given filters.
fn setup_table_blueprint(test_context: &TestContext, filters: &[String]) -> TableBlueprints {
    let blueprint_id = StoreId::random(StoreKind::Blueprint, "table-blueprint-test");
    let mut store_hub = test_context.store_hub.lock();
    write_filters(
        store_hub.store_bundle_mut().blueprint_entry(&blueprint_id),
        filters,
    );

    let mut table_blueprints = TableBlueprints::default();
    table_blueprints
        .set_default_blueprint(
            &TableReference::local("test_table"),
            &blueprint_id,
            &mut store_hub,
        )
        .expect("default table blueprint should be set");
    table_blueprints
}

/// Writes the filters to the active table blueprint.
fn write_active_filters(
    test_context: &TestContext,
    table_blueprints: &TableBlueprints,
    filters: &[String],
) {
    let active_id = table_blueprints
        .active_id(&TableReference::local("test_table"))
        .expect("table should have an active blueprint");
    let mut store_hub = test_context.store_hub.lock();
    write_filters(
        store_hub.store_bundle_mut().blueprint_entry(active_id),
        filters,
    );
}

fn write_filters(store: &mut EntityDb, filters: &[String]) {
    let timepoint = blueprint_timepoint_for_writes(store);
    let chunk = Arc::new(
        Chunk::builder("table")
            .with_archetype_auto_row(
                timepoint,
                &TableBlueprint::update_fields().with_filters(
                    filters
                        .iter()
                        .map(|sql| SqlFilterExpression::from(sql.as_str())),
                ),
            )
            .build()
            .expect("filter chunk should build"),
    );
    store
        .add_chunk(&chunk)
        .expect("filter chunk should be added");
}

/// The filters in the active table blueprint.
fn blueprint_filters(
    test_context: &TestContext,
    table_blueprints: &TableBlueprints,
) -> Vec<String> {
    let active_id = table_blueprints
        .active_id(&TableReference::local("test_table"))
        .expect("table should have an active blueprint");
    let store_hub = test_context.store_hub.lock();
    let store = store_hub
        .store_bundle()
        .get(active_id)
        .expect("active blueprint should exist");
    let component = TableBlueprint::descriptor_filters().component;
    store
        .latest_at(
            &re_chunk_store::LatestAtQuery::latest(re_viewer_context::blueprint_timeline()),
            &"/table".into(),
            [component],
        )
        .component_batch::<SqlFilterExpression>(component)
        .unwrap_or_default()
        .into_iter()
        .map(|filter| filter.as_str().to_owned())
        .collect()
}

/// Sets up a table with an integer and a timestamp column.
fn setup_test_table() -> (Arc<SessionContext>, &'static str) {
    let schema = Arc::new(Schema::new_with_metadata(
        vec![Field::new("a", DataType::Int64, false), timestamp_field()],
        Default::default(),
    ));
    common::register_test_table(
        "test_table",
        schema,
        vec![
            Arc::new(Int64Array::from(vec![1, 2, 3])),
            Arc::new(TimestampNanosecondArray::from(vec![0, 1, 2]).with_timezone("UTC")),
        ],
    )
}
