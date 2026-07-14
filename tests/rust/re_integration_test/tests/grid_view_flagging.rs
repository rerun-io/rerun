//! Integration test for grid view flag toggling with server persistence.
//!
//! Verifies that flag buttons appear in grid view for a remote table and that
//! clicking a flag visually toggles it.
//!
//! See also: `re_dataframe_ui::tests::grid_view::test_grid_view_flagging` for
//! the widget-level (in-memory only) version of this test.

use std::sync::Arc;
use std::time::Duration;

use arrow::array::{AsArray as _, BooleanArray, Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use futures::StreamExt as _;
use re_integration_test::{HarnessExt as _, TestServer};
use re_protos::cloud::v1alpha1::ScanTableRequest;
use re_protos::cloud::v1alpha1::ext::TableInsertMode;
use re_sdk::external::re_log_types;
use re_viewer::viewer_test_utils::{self, HarnessOptions};

#[tokio::test(flavor = "multi_thread")]
pub async fn grid_view_flagging() {
    let server = TestServer::spawn().await;
    let mut client = server.client().await.expect("Failed to connect to server");

    // Create a table with a boolean flag column and a table index.
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("id", DataType::Int64, false)
                .with_metadata([("rerun:is_table_index".to_owned(), "true".to_owned())].into()),
            Field::new("name", DataType::Utf8, false),
            Field::new("flagged", DataType::Boolean, true).with_metadata(
                [(
                    re_dataframe_ui::experimental_field_metadata::IS_FLAG_COLUMN.to_owned(),
                    "true".to_owned(),
                )]
                .into(),
            ),
        ],
        Default::default(),
    ));
    let batch = RecordBatch::try_new_with_options(
        schema.clone(),
        vec![
            Arc::new(Int64Array::from(vec![1, 2, 3])),
            Arc::new(StringArray::from(vec!["Alice", "Bob", "Charlie"])),
            Arc::new(BooleanArray::from(vec![false, false, false])),
        ],
        &Default::default(),
    )
    .unwrap();

    let table = client
        .create_table_entry(
            re_log_types::EntryName::new("flag_test").unwrap(),
            None,
            schema,
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

    // Open the viewer directly at the table entry.
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        startup_url: Some(format!(
            "rerun+http://localhost:{}/entry/{}",
            server.port(),
            table.details.id
        )),
        ..Default::default()
    });
    viewer_test_utils::step_until(
        "table data loads",
        &mut harness,
        |harness| harness.query_by_label_contains("Alice").is_some(),
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
    harness.set_blueprint_panel_opened(false);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(false);

    // Switch to grid mode.
    harness.get_by_label("Grid view").click();
    harness.run_ok();

    // Wait for flag buttons to appear.
    viewer_test_utils::step_until(
        "grid view renders with flag buttons",
        &mut harness,
        |harness| {
            harness
                .query_all_by_role_and_label(Role::CheckBox, "Flag")
                .next()
                .is_some()
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );

    harness.snapshot("grid_view_flagging_before");

    // Toggle Alice's flag (first checkbox).
    harness
        .query_all_by_role_and_label(Role::CheckBox, "Flag")
        .next()
        .expect("flag button should be present")
        .click();
    harness.run_ok();

    harness.snapshot("grid_view_flagging_after");

    // Wait for the async upsert to reach the server, then verify the flag was persisted.
    viewer_test_utils::step_until(
        "flag upsert persisted to server",
        &mut harness,
        |_harness| {
            // Read back from the server via scan_table.
            // We're inside a multi-thread tokio runtime, so we can block on async here.
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(async { scan_flag_value(&server, table.details.id, 1).await })
            }) == Some(true)
        },
        Duration::from_millis(100),
        Duration::from_secs(5),
    );
}

/// Read back flag value at a specific row from the server by scanning the table.
///
/// Returns `None` if we failed to connect or the target id wasn't found.
async fn scan_flag_value(
    server: &TestServer,
    table_id: re_log_types::EntryId,
    target_id: i64,
) -> Option<bool> {
    let mut client = server.client().await.ok()?;
    let response = client
        .inner()
        .scan_table(ScanTableRequest {
            table_id: Some(table_id.into()),
        })
        .await
        .ok()?
        .into_inner();

    futures::pin_mut!(response);
    while let Some(Ok(resp)) = response.next().await {
        if let Some(part) = resp.dataframe_part
            && let Ok(batch) = RecordBatch::try_from(part)
        {
            let id_col: &arrow::array::Int64Array = batch.column_by_name("id")?.as_primitive();
            let flag_col = batch.column_by_name("flagged")?.as_boolean();

            for row in 0..batch.num_rows() {
                if id_col.value(row) == target_id {
                    return Some(flag_col.value(row));
                }
            }
        }
    }

    None
}
