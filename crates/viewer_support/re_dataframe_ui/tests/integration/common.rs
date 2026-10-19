use std::sync::Arc;
use std::time::{Duration, Instant};

use arrow::array::ArrayRef;
use arrow::datatypes::SchemaRef;
use arrow::record_batch::RecordBatch;
use datafusion::prelude::SessionContext;
use egui::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use re_chunk_store::LatestAtQuery;
use re_chunk_store::external::re_chunk::Chunk;
use re_dataframe_ui::TableBlueprints;
use re_entity_db::EntityDb;
use re_log_types::{StoreId, StoreKind};
use re_sdk_types::blueprint::archetypes::TableBlueprint;
use re_test_context::TestContext;
use re_types_core::{Component, ComponentIdentifier};
use re_viewer_context::{TableReference, blueprint_timeline, blueprint_timepoint_for_writes};

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

/// Sets up a default table blueprint for `test_table` with the given fields.
pub fn setup_table_blueprint(
    test_context: &TestContext,
    fields: &TableBlueprint,
) -> TableBlueprints {
    let blueprint_id = StoreId::random(StoreKind::Blueprint, "table-blueprint-test");
    let mut store_hub = test_context.store_hub.lock();
    write_table_blueprint(
        store_hub.store_bundle_mut().blueprint_entry(&blueprint_id),
        fields,
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

/// Writes the fields to the active table blueprint of `test_table`.
pub fn write_active_table_blueprint(
    test_context: &TestContext,
    table_blueprints: &TableBlueprints,
    fields: &TableBlueprint,
) {
    let active_id = active_table_blueprint_id(table_blueprints);
    let mut store_hub = test_context.store_hub.lock();
    write_table_blueprint(
        store_hub.store_bundle_mut().blueprint_entry(active_id),
        fields,
    );
}

/// Reads one component of the active table blueprint of `test_table`.
pub fn read_active_table_blueprint<C: Component>(
    test_context: &TestContext,
    table_blueprints: &TableBlueprints,
    component: ComponentIdentifier,
) -> Option<Vec<C>> {
    let active_id = active_table_blueprint_id(table_blueprints);
    let store_hub = test_context.store_hub.lock();
    let store = store_hub
        .store_bundle()
        .get(active_id)
        .expect("active table blueprint should exist");
    store
        .latest_at(
            &LatestAtQuery::latest(blueprint_timeline()),
            &"/table".into(),
            [component],
        )
        .component_batch::<C>(component)
}

fn active_table_blueprint_id(table_blueprints: &TableBlueprints) -> &StoreId {
    table_blueprints
        .active_id(&TableReference::local("test_table"))
        .expect("table should have an active blueprint")
}

fn write_table_blueprint(store: &mut EntityDb, fields: &TableBlueprint) {
    let chunk = Arc::new(
        Chunk::builder("table")
            .with_archetype_auto_row(blueprint_timepoint_for_writes(store), fields)
            .build()
            .expect("table blueprint chunk should build"),
    );
    store
        .add_chunk(&chunk)
        .expect("table blueprint chunk should be added");
}
