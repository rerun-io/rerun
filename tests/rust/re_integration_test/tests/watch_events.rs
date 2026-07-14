use std::sync::Arc;
use std::time::Duration;

use arrow::array::{Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use egui_kittest::SnapshotResults;
use egui_kittest::kittest::Queryable as _;
use re_integration_test::{HarnessExt as _, TestServer};
use re_protos::cloud::v1alpha1::EntryFilter;
use re_protos::cloud::v1alpha1::ext::{self, TableInsertMode};
use re_sdk::external::re_log_types;
use re_viewer::viewer_test_utils::{self, HarnessOptions};

/// The viewer should auto-refresh its catalog when entries are added/removed server-side,
/// driven by the `WatchEvents` stream (no manual refresh).
#[tokio::test(flavor = "multi_thread")]
pub async fn watch_events_auto_refresh_test() {
    let server = TestServer::spawn().await;
    let mut client = server.client().await.expect("Failed to connect to server");

    let transient_dataset = "my_dataset";
    let persistent_dataset = "persistent_dataset";
    let transient_table = "my_table";
    let persistent_table = "persistent_table";

    // Create a persistent dataset and table up front as stable reference points that stay
    // around across the create/delete below.
    let persistent = client
        .create_dataset_entry(persistent_dataset.to_owned(), None)
        .await
        .expect("Failed to create persistent dataset");
    create_table(&mut client, persistent_table).await;

    // Open the viewer *directly at* the persistent dataset. This connects to the server,
    // which spawns the `WatchEvents` listener for this origin.
    let dataset_url = format!(
        "rerun+http://localhost:{}/entry/{}",
        server.port(),
        persistent.details.id
    );
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        startup_url: Some(dataset_url),
        ..Default::default()
    });
    let mut snapshot_results = SnapshotResults::new();

    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(false);

    // Wait for the persistent dataset and table to appear in the panel.
    viewer_test_utils::step_until(
        "Persistent entries appear",
        &mut harness,
        |harness| {
            let panel = harness.recording_panel();
            let root = panel.root();
            root.query_by_label_contains(persistent_dataset).is_some()
                && root.query_by_label_contains(persistent_table).is_some()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );

    // Select the persistent table so its data is shown, avoiding the transient "Loading…" state
    // of the persistent dataset in the snapshot.
    harness
        .get_all_by_label(persistent_table)
        .next()
        .expect("persistent table label should be present")
        .click();
    harness.run_ok();
    viewer_test_utils::step_until(
        "Persistent table data is rendered in main view",
        &mut harness,
        |harness| harness.query_by_label_contains("alpha").is_some(),
        Duration::from_millis(100),
        Duration::from_secs(10),
    );
    snapshot_results.add(harness.try_snapshot("watch_events_1_initial"));

    // When creating entries, the server emits `EntryCreated` and the viewer's watch loop
    // auto-refreshes the catalog without a manual refresh.
    let dataset = client
        .create_dataset_entry(transient_dataset.to_owned(), None)
        .await
        .expect("Failed to create dataset");
    let table = create_table(&mut client, transient_table).await;

    viewer_test_utils::step_until(
        "Transient entries auto-appear",
        &mut harness,
        |harness| {
            let panel = harness.recording_panel();
            let root = panel.root();
            root.query_by_label_contains(transient_dataset).is_some()
                && root.query_by_label_contains(transient_table).is_some()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
    viewer_test_utils::step_until(
        "Persistent table data is rendered again after refresh",
        &mut harness,
        |harness| harness.query_by_label_contains("alpha").is_some(),
        Duration::from_millis(100),
        Duration::from_secs(10),
    );
    snapshot_results.add(harness.try_snapshot("watch_events_2_entries_added"));

    // Open the transient table so its data is the currently-viewed content when it gets deleted
    // below. Rows contain the string "alpha" (from the first data batch). Pick the first match,
    // which is the entry in the left panel.
    harness
        .get_all_by_label(transient_table)
        .next()
        .expect("transient table label should be present")
        .click();
    harness.run_ok();
    viewer_test_utils::step_until(
        "Transient table data is rendered in main view",
        &mut harness,
        |harness| harness.query_by_label_contains("alpha").is_some(),
        Duration::from_millis(100),
        Duration::from_secs(10),
    );

    // When deleting entries, the server emits `EntryDeleted` and the viewer auto-refreshes again.
    client
        .delete_entry(dataset.details.id)
        .await
        .expect("Failed to delete dataset");
    client
        .delete_entry(table.details.id)
        .await
        .expect("Failed to delete table");

    // Sanity check: confirm the server really dropped them.
    for name in [transient_dataset, transient_table] {
        let remaining = client
            .find_entries(EntryFilter {
                id: None,
                name: Some(name.to_owned()),
                entry_kind: None,
            })
            .await
            .expect("find_entries failed");
        assert!(
            remaining.is_empty(),
            "entry still exists server-side after delete: {name} {remaining:?}"
        );
    }

    // The deleted entries auto-disappear on their own while the persistent ones stay around.
    viewer_test_utils::step_until(
        "Transient entries auto-disappear",
        &mut harness,
        |harness| {
            let panel = harness.recording_panel();
            let root = panel.root();
            root.query_by_label_contains(transient_dataset).is_none()
                && root.query_by_label_contains(transient_table).is_none()
                && root.query_by_label_contains(persistent_dataset).is_some()
                && root.query_by_label_contains(persistent_table).is_some()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
    snapshot_results.add(harness.try_snapshot("watch_events_3_entries_removed"));
}

/// Creates a table entry and writes an initial data batch (rows `alpha`/`beta`/`gamma`).
async fn create_table(
    client: &mut re_redap_client::ConnectionClient,
    name: &str,
) -> ext::TableEntry {
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("id", DataType::Int64, false),
            Field::new("name", DataType::Utf8, false),
        ],
        Default::default(),
    ));
    let batch = RecordBatch::try_new_with_options(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![1, 2, 3])),
            Arc::new(StringArray::from(vec!["alpha", "beta", "gamma"])),
        ],
        &Default::default(),
    )
    .expect("Failed to create record batch");
    let table = client
        .create_table_entry(
            re_log_types::EntryName::new(name).expect("Failed to create entry name"),
            None,
            schema.clone(),
        )
        .await
        .expect("Failed to create table");
    client
        .write_table(
            futures::stream::once(async { batch }),
            table.details.id,
            TableInsertMode::Append,
        )
        .await
        .expect("Failed to write initial data");
    table
}
