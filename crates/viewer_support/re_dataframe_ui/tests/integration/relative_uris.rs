//! Test a column whose values are Rerun URIs written relative to the table's own server.

use crate::common;

use std::sync::Arc;

use arrow::array::StringArray;
use arrow::datatypes::{DataType, Field, Schema};
use datafusion::prelude::SessionContext;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use re_async::AsyncRuntimeHandle;
use re_chunk_store::external::re_chunk::Chunk;
use re_dataframe_ui::{DataFusionTableWidget, TableBlueprints};
use re_log_types::{StoreId, StoreKind};
use re_sdk_types::blueprint::archetypes::{TableColumn, TableLayout};
use re_sdk_types::blueprint::components::TableCellKind;
use re_test_context::TestContext;
use re_viewer_context::{Route, TableReference, blueprint_timepoint_for_writes};

use common::run_async_harness;

const DATASET_ID: &str = "1830B33B45B963E7774455beb91701ae";

/// A relative reference is the path of a URI on the route's server.
const RELATIVE_URI: &str = "/dataset/1830B33B45B963E7774455beb91701ae?segment_id=my_segment";

/// An absolute uri in the same column, naming a different server than the route's.
const ABSOLUTE_URI: &str =
    "rerun+http://example.com:9999/dataset/1830B33B45B963E7774455beb91701ae?segment_id=other";

/// A value that is no reference at all renders as plain text rather than a link.
const MALFORMED_URI: &str = "not a uri";

/// Under a route on a server, relative references resolve and every other value is left alone.
#[tokio::test]
async fn test_relative_uris_resolve_against_the_route_origin() {
    let entry_uri: re_uri::EntryUri =
        "rerun+http://localhost:1234/entry/00000000000000000000000000000001"
            .parse()
            .expect("test entry URI should be valid");

    let route = Route::RedapEntry {
        origin: entry_uri.origin.clone(),
        entry_id: entry_uri.entry_id,
        kind: None,
    };
    let buttons = run_uri_table(TableReference::from(entry_uri), Some(route)).await;

    assert!(buttons.contains(&format!(
        "rerun+http://localhost:1234/dataset/{DATASET_ID}?segment_id=my_segment"
    )));
    assert!(buttons.contains(&ABSOLUTE_URI.to_owned()));
    assert!(!buttons.iter().any(|label| label.contains(MALFORMED_URI)));
}

/// A route with no origin leaves relative references as they are, so they render as text.
#[tokio::test]
async fn test_relative_uris_are_left_alone_without_an_origin() {
    let buttons = run_uri_table(TableReference::local("uri_table"), None).await;

    assert!(!buttons.iter().any(|label| label.contains("my_segment")));
    assert!(buttons.contains(&ABSOLUTE_URI.to_owned()));
}

/// Render the uri table and return the label of every link button it shows.
async fn run_uri_table(table_ref: TableReference, route: Option<Route>) -> Vec<String> {
    let (session_context, datafusion_table_ref) = setup_uri_table();

    let mut test_context = TestContext::new();
    test_context.route = route;
    test_context.component_ui_registry = re_component_ui::create_component_ui_registry();
    let runtime_handle = AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()
        .expect("test should run inside its Tokio runtime");
    let table_blueprints = setup_uri_blueprint(&test_context, &table_ref);

    let mut harness = test_context
        .setup_kittest_for_rendering_ui([1200.0, 400.0])
        .build_ui(|ui| {
            test_context.run_recording(&ui.ctx().clone(), |ctx| {
                DataFusionTableWidget::new(
                    Arc::clone(&session_context),
                    datafusion_table_ref,
                    table_ref.clone(),
                )
                .title("Relative uris")
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
        .query_all_by_role(Role::Button)
        .filter_map(|node| node.accesskit_node().label())
        .collect()
}

fn setup_uri_blueprint(test_context: &TestContext, table_ref: &TableReference) -> TableBlueprints {
    let blueprint_id = StoreId::random(StoreKind::Blueprint, "relative-uri-test");
    let mut store_hub = test_context.store_hub.lock();
    let store = store_hub.store_bundle_mut().blueprint_entry(&blueprint_id);
    let timepoint = blueprint_timepoint_for_writes(store);

    for chunk in [
        Chunk::builder("table/layouts/table")
            .with_archetype_auto_row(
                timepoint.clone(),
                &TableLayout::new().with_column_order(["name", "uri"]),
            )
            .build()
            .expect("table layout chunk should build"),
        Chunk::builder("table/layouts/table/columns/uri")
            .with_archetype_auto_row(
                timepoint.clone(),
                &TableColumn::new().with_cell_kind(TableCellKind::Link),
            )
            .build()
            .expect("column chunk should build"),
    ] {
        store
            .add_chunk(&Arc::new(chunk))
            .expect("blueprint chunk should be added");
    }

    let mut table_blueprints = TableBlueprints::default();
    table_blueprints
        .set_default_blueprint(table_ref, &blueprint_id, &mut store_hub)
        .expect("default table blueprint should be set");
    table_blueprints
}

fn setup_uri_table() -> (Arc<SessionContext>, &'static str) {
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("name", DataType::Utf8, false),
            Field::new("uri", DataType::Utf8, false),
        ],
        Default::default(),
    ));

    common::register_test_table(
        "uri_table",
        schema,
        vec![
            Arc::new(StringArray::from(vec!["relative", "absolute", "malformed"])),
            Arc::new(StringArray::from(vec![
                RELATIVE_URI,
                ABSOLUTE_URI,
                MALFORMED_URI,
            ])),
        ],
    )
}
