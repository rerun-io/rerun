//! Integration test for boolean editing in table and card layouts with server persistence.

use std::sync::Arc;

use arrow::array::{AsArray as _, BooleanArray, Int64Array, RecordBatch, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use futures::StreamExt as _;
use re_integration_test::TestServer;
use re_integration_test::ViewerHarnessExt as _;
use re_protos::cloud::v1alpha1::ScanTableRequest;
use re_protos::cloud::v1alpha1::ext::TableInsertMode;
use re_sdk::RecordingStreamBuilder;
use re_sdk::external::re_log_types;
use re_sdk_types::blueprint::archetypes::{CardLayout, TableColumn, TableLayout};
use re_sdk_types::blueprint::components::TableCellKind;
use re_viewer::viewer_test_utils::{self, HarnessOptions};

#[tokio::test(flavor = "multi_thread")]
pub async fn cards_view_flagging() {
    let server = TestServer::spawn().await;
    let mut client = server.client().await.expect("Failed to connect to server");

    // Create a table with a boolean flag column and a table index.
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("id", DataType::Int64, false)
                .with_metadata([("rerun:is_table_index".to_owned(), "true".to_owned())].into()),
            Field::new("name", DataType::Utf8, false),
            Field::new("flagged", DataType::Boolean, true),
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

    let blueprint_rbl = flag_blueprint_rbl_file();
    re_integration_test::register_table_blueprint(
        &server.connection_handle(),
        &table,
        blueprint_rbl.path(),
    )
    .await
    .expect("Failed to register table blueprint");

    // Open the viewer directly at the table entry.
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        startup_url: Some(format!(
            "rerun+http://localhost:{}/entry/{}",
            server.port(),
            table.details.id
        )),
        ..Default::default()
    });
    viewer_test_utils::step_until("table data loads", &mut harness, |harness| {
        harness.query_by_label_contains("Alice").is_some()
    });
    harness.set_blueprint_panel_opened(false);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(false);

    // Edit the first row in table layout, then wait for the asynchronous upsert.
    harness.get_by_label("Table view").click();
    harness.run_ok();

    viewer_test_utils::step_until(
        "table layout renders with flag buttons",
        &mut harness,
        |harness| {
            harness
                .query_all_by_role_and_label(Role::CheckBox, "Flag")
                .next()
                .is_some()
        },
    );

    harness.snapshot("boolean_editing_table_before");

    harness
        .query_all_by_role_and_label(Role::CheckBox, "Flag")
        .next()
        .expect("flag button should be present")
        .click();
    harness.run_ok();

    harness.snapshot("boolean_editing_table_after");

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
    );

    // Switching layouts must show the server-backed value before the card edits it again.
    harness.get_by_label("Cards view").click();
    harness.run_ok();
    viewer_test_utils::step_until(
        "card layout shows the persisted edit",
        &mut harness,
        |harness| {
            harness
                .query_all_by_role_and_label(Role::CheckBox, "Flag")
                .next()
                .is_some()
        },
    );
    harness.snapshot("boolean_editing_cards_before");

    harness
        .query_all_by_role_and_label(Role::CheckBox, "Flag")
        .next()
        .expect("flag button should be present")
        .click();
    harness.run_ok();
    harness.snapshot("boolean_editing_cards_after");

    // The card edit uses the same index-plus-value upsert path and restores the initial value.
    viewer_test_utils::step_until("card edit persisted to server", &mut harness, |_harness| {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { scan_flag_value(&server, table.details.id, 1).await })
        }) == Some(false)
    });
}

fn flag_blueprint_rbl_file() -> tempfile::NamedTempFile {
    let file = tempfile::Builder::new()
        .suffix(".rbl")
        .tempfile()
        .expect("Failed to create blueprint temp file");

    let stream = RecordingStreamBuilder::new("rerun_example_table_flag_blueprint")
        .blueprint()
        .save(file.path())
        .expect("Failed to create blueprint stream");
    stream.set_time_sequence("blueprint", 0);
    stream
        .log(
            "table/layouts/table/columns/flagged",
            &TableColumn::new()
                .with_editable(true)
                .with_cell_kind(TableCellKind::Flag),
        )
        .expect("Failed to log table column blueprint");
    stream
        .log(
            "table/layouts/table",
            &TableLayout::new().with_column_order(["name", "flagged"]),
        )
        .expect("Failed to log table layout");
    stream
        .log(
            "table/layouts/cards/fields/flagged",
            &TableColumn::new()
                .with_editable(true)
                .with_cell_kind(TableCellKind::Flag),
        )
        .expect("Failed to log flag field blueprint");
    stream
        .log(
            "table/layouts/cards",
            &CardLayout::new(["flagged"]).with_title("name"),
        )
        .expect("Failed to log card layout");

    file
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
