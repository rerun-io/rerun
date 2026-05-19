//! Reproduces the scenario where:
//! 1. Two tables are created on a server.
//! 2. The viewer is opened directly at one of the tables (so its content is on screen).
//! 3. That table is deleted server-side while the viewer is still viewing it.
//! 4. The user triggers a table refresh from the bottom panel.
//!
//! Expected: the user sees an error and there is no infinite loop

use std::sync::Arc;
use std::time::Duration;

use arrow::array::{Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use egui_kittest::kittest::Queryable as _;
use re_integration_test::TestServer;
use re_protos::cloud::v1alpha1::ext::TableInsertMode;
use re_sdk::external::re_log_types;
use re_viewer::viewer_test_utils::{self, HarnessOptions};

const DELETED_TABLE: &str = "rr4424_to_delete";
const PERSISTENT_TABLE: &str = "rr4424_keeps_around";

#[tokio::test(flavor = "multi_thread")]
pub async fn deleted_table_refresh() {
    let server = TestServer::spawn().await;
    let mut client = server.client().await.expect("Failed to connect to server");

    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("id", DataType::Int64, false),
            Field::new("name", DataType::Utf8, false),
        ],
        Default::default(),
    ));

    // Create two tables so we have a stable reference point to verify the
    // refresh actually reloaded the list (vs. just rendering empty transiently).
    let to_delete = create_table(&mut client, DELETED_TABLE, &schema).await;
    let _persistent = create_table(&mut client, PERSISTENT_TABLE, &schema).await;

    // Open the viewer *directly at the table* that we're going to delete, so
    // it is the currently-viewed entry when the delete + refresh happens.
    let table_url = format!(
        "rerun+http://localhost:{}/entry/{}",
        server.port(),
        to_delete.details.id
    );
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        startup_url: Some(table_url),
        ..Default::default()
    });

    // Wait for the table's data to load in the main view — rows in the table
    // contain the string "alpha" (from the first data batch).
    viewer_test_utils::step_until(
        "table data is rendered in main view",
        &mut harness,
        |harness| harness.query_by_label_contains("alpha").is_some(),
        Duration::from_millis(100),
        Duration::from_secs(10),
    );

    // Delete the currently-viewed table server-side.
    client
        .delete_entry(to_delete.details.id)
        .await
        .expect("Failed to delete table");

    // Sanity check: confirm the server really dropped it.
    let remaining = client
        .find_entries(re_protos::cloud::v1alpha1::EntryFilter {
            id: None,
            name: Some(DELETED_TABLE.to_owned()),
            entry_kind: None,
        })
        .await
        .expect("find_entries failed");
    assert!(
        remaining.is_empty(),
        "table still exists server-side after delete: {remaining:?}"
    );

    // Trigger a refresh via the bottom-right "Refresh table" button on the
    // currently-viewed (now deleted) table widget.
    harness.get_by_label("Refresh table").click();

    // After refreshing the (now deleted) table itself, we expect the widget to
    // surface a "Could not load table" error rather than hang or spam logs.
    // Also wait for the error toasts (which contain the same non-deterministic
    // entry id + trace-id) to auto-expire so only the inline error widget
    // needs masking.
    viewer_test_utils::step_until(
        "deleted table shows an error and toasts have expired",
        &mut harness,
        |harness| {
            harness
                .query_by_label_contains("Could not load table")
                .is_some()
                && harness
                    .query_by_label_contains("DataFusion query error")
                    .is_none()
        },
        Duration::from_millis(100),
        Duration::from_secs(15),
    );

    // The inline error widget's text embeds a freshly-generated entry id and
    // gRPC trace-id, so mask the whole widget before snapshotting.
    let rects_to_mask: Vec<egui::Rect> = harness
        .query_all_by_label_contains("Could not load table")
        .map(|node| node.rect())
        .collect();
    for rect in rects_to_mask {
        harness.mask(rect);
    }

    harness.snapshot("deleted_table_refresh_should_show_error");
}

async fn create_table(
    client: &mut re_redap_client::ConnectionClient,
    name: &str,
    schema: &Arc<Schema>,
) -> re_protos::cloud::v1alpha1::ext::TableEntry {
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
