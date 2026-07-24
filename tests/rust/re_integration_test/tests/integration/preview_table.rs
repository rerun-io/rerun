//! End-to-end test for table segment previews.
//!
//! Builds a remote table whose rows carry a recording URI and an embedded table blueprint
//! that defines a `Spatial3DView`. The viewer loads the referenced recording on demand and
//! renders it inline in the table's sticky preview column. We then verify both that the
//! recording was actually loaded and that the rendered preview matches a snapshot.
//!
//! The referenced recording logs its `Points3D` statically, so the preview renders the same
//! at every point on its looping preview timeline and the snapshot stays stable.

use std::str::FromStr as _;
use std::sync::Arc;
use std::time::Duration;

use arrow::array::{Int64Array, RecordBatch, RecordBatchOptions, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use egui_kittest::kittest::Queryable as _;

use re_integration_test::{HarnessExt as _, TestServer};
use re_sdk::RecordingStreamBuilder;
use re_sdk::external::{re_log_types, re_tuid};
use re_sdk_types::blueprint::archetypes::{
    ContainerBlueprint, TableBlueprint, ViewBlueprint, ViewContents, ViewportBlueprint,
};
use re_sdk_types::blueprint::components::{
    ContainerKind, IncludedContent, QueryExpression, RootContainer, ViewClass,
};
use re_viewer::viewer_test_utils::{self, HarnessOptions};

const DATASET_ID: &str = "187b552b95a5c2f73f37894708825ba5";
const PREVIEW_COLUMN: &str = "recording_uri";
const TITLE_COLUMN: &str = "name";
const SEGMENT_COUNT: usize = 4;

#[tokio::test(flavor = "multi_thread")]
pub async fn preview_table() {
    let (server, segment_ids) = TestServer::spawn()
        .await
        .with_static_preview_data(
            "preview_dataset",
            DATASET_ID,
            "preview_recording",
            SEGMENT_COUNT,
        )
        .await;

    // One row per segment, each pointing at its segment's recording URI.
    let dataset_id = re_tuid::Tuid::from_str(DATASET_ID).expect("Failed to parse TUID");
    let segment_uris: Vec<re_uri::DatasetSegmentUri> = segment_ids
        .iter()
        .map(|segment_id| re_uri::DatasetSegmentUri {
            origin: re_uri::Origin {
                scheme: re_uri::Scheme::RerunHttp,
                host: re_uri::external::url::Host::Domain("localhost".to_owned()),
                port: server.port(),
            },
            dataset_id,
            segment_id: segment_id.clone(),
            fragment: Default::default(),
        })
        .collect();

    // Create a remote table with a recording-URI column. A registered blueprint (set up below)
    // renders each recording in a 3D preview. The `name` column gives grid-view cards stable
    // titles.
    let schema = Arc::new(Schema::new_with_metadata(
        vec![
            Field::new("id", DataType::Int64, false)
                .with_metadata([("rerun:is_table_index".to_owned(), "true".to_owned())].into()),
            Field::new(TITLE_COLUMN, DataType::Utf8, false),
            Field::new(PREVIEW_COLUMN, DataType::Utf8, false),
        ],
        Default::default(),
    ));

    let mut client = server.client().await.expect("Failed to connect to server");
    let table = client
        .create_table_entry(
            re_log_types::EntryName::new("preview_table").expect("valid entry name"),
            None,
            schema.clone(),
        )
        .await
        .expect("Failed to create table");

    let names: Vec<String> = (0..SEGMENT_COUNT).map(|i| format!("segment {i}")).collect();
    let batch = RecordBatch::try_new_with_options(
        schema,
        vec![
            Arc::new(Int64Array::from_iter_values(
                (0..SEGMENT_COUNT).map(|i| i64::try_from(i).expect("segment index fits in i64")),
            )),
            Arc::new(StringArray::from(names)),
            Arc::new(StringArray::from(
                segment_uris
                    .iter()
                    .map(|uri| uri.to_string())
                    .collect::<Vec<_>>(),
            )),
        ],
        &RecordBatchOptions::new().with_row_count(Some(SEGMENT_COUNT)),
    )
    .expect("Failed to build table batch");
    client
        .write_table(
            futures::stream::once(async { batch }),
            table.details.id,
            re_protos::cloud::v1alpha1::ext::TableInsertMode::Append,
        )
        .await
        .expect("Failed to write table data");

    // Register the table blueprint with the table's implicit blueprint dataset and set it as the default.
    let blueprint_rbl = blueprint_rbl_file(PREVIEW_COLUMN, TITLE_COLUMN);
    re_integration_test::register_table_blueprint(&mut client, &table, blueprint_rbl.path())
        .await
        .expect("Failed to register table blueprint");

    // Open the viewer directly at the table entry. Make the window tall enough that all rows
    // are on screen at once, so every preview loads.
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::vec2(1024.0, 1000.0)),
        startup_url: Some(format!(
            "rerun+http://localhost:{}/entry/{}",
            server.port(),
            table.details.id
        )),
        ..Default::default()
    });

    // Step until every preview recording has actually streamed in its point data. Rendering the
    // preview column is what triggers the background loads, so this also exercises the column.
    let preview_uris: Vec<re_uri::DatasetSegmentUri> = segment_uris
        .iter()
        .map(|uri| uri.clone().without_fragment())
        .collect();
    let preview_entity = re_log_types::EntityPath::from("test_entity");
    viewer_test_utils::step_until(
        "All preview recordings loaded",
        &mut harness,
        |harness| {
            let uris = preview_uris.clone();
            let entity = preview_entity.clone();
            harness.run_with_app_context(move |app_context| {
                uris.iter().all(|uri| {
                    app_context
                        .storage_context
                        .hub
                        .find_recording_by_uri(uri)
                        .is_some_and(|db| {
                            // Not only needs the recording be loaded, we also need the data to arrive.
                            db.storage_engine()
                                .store()
                                .entity_has_physical_static_data(&entity)
                        })
                })
            })
        },
        Duration::from_millis(100),
        Duration::from_secs(30),
    );

    // Let the 3D views' camera framing settle before snapshotting.
    harness.run_ok();
    harness.snapshot("preview_table");

    // Switch to grid view and snapshot the same previews as cards.
    harness.get_by_label("Grid view").click();
    harness.run_ok();
    harness.snapshot("preview_table_grid");

    // Clicking the first card opens its recording, navigating away from the table.
    //
    // We have to drive this click by hand rather than via `.click()` / `click_at`:
    // - `click_at` calls `run()`, which never settles while the previews keep repainting.
    // - `.click()` presses and releases in a single frame, which the card's click region
    //   doesn't register.
    // The card registers its click area behind its content, so a click on the title label or
    // the preview never reaches it. We click the empty space to the right of the title.
    let title = harness.get_by_label("segment 0").rect();
    let click_pos = egui::pos2(title.right() + 150.0, title.center().y);
    harness.event(egui::Event::PointerMoved(click_pos));
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: click_pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.step();
    }

    let opened_segment = preview_uris[0].clone();
    viewer_test_utils::step_until(
        "Clicked card opens its recording",
        &mut harness,
        |harness| {
            let uri = opened_segment.clone();
            harness.run_with_app_context(move |app_context| {
                let expected = app_context
                    .storage_context
                    .hub
                    .find_recording_by_uri(&uri)
                    .map(|db| db.store_id().clone());
                expected.is_some() && app_context.route.recording_id().cloned() == expected
            })
        },
        Duration::from_millis(100),
        Duration::from_secs(15),
    );

    viewer_test_utils::step_until(
        "Opened recording finished loading",
        &mut harness,
        |harness| {
            harness.query_by_label_contains("Streams").is_some()
                && harness.query_by_label("Loading entries…").is_none()
        },
        Duration::from_millis(100),
        Duration::from_secs(15),
    );
    // Close the selection panel rather than masking it: it shows the recording URI, which
    // embeds the server's random port.
    harness.set_selection_panel_opened(false);
    harness.mask_dates();
    harness.snapshot("preview_table_opened_recording");
}

/// Build a `.rbl` blueprint file holding a `Spatial3DView` over `/test_entity` plus a
/// `TableBlueprint` archetype pointing segment previews at `preview_column` and grid-view card
/// titles at `title_column`.
fn blueprint_rbl_file(preview_column: &str, title_column: &str) -> tempfile::NamedTempFile {
    let file = tempfile::Builder::new()
        .suffix(".rbl")
        .tempfile()
        .expect("Failed to create blueprint temp file");

    let stream = RecordingStreamBuilder::new("rerun_example_table_blueprint")
        .blueprint()
        .save(file.path())
        .expect("Failed to create blueprint memory stream");
    stream.set_time_sequence("blueprint", 0);

    let view_id = uuid::Uuid::new_v4();
    let view_path = format!("view/{view_id}");
    stream
        .log(
            format!("{view_path}/ViewContents"),
            &ViewContents::new([QueryExpression("/test_entity/**".into())]),
        )
        .expect("Failed to log view contents");
    stream
        .log(
            view_path.clone(),
            &ViewBlueprint::new(ViewClass("3D".into())).with_space_origin("/test_entity"),
        )
        .expect("Failed to log view blueprint");

    let container_id = uuid::Uuid::new_v4();
    stream
        .log(
            format!("container/{container_id}"),
            &ContainerBlueprint::new(ContainerKind::Tabs)
                .with_contents([IncludedContent(view_path.into())]),
        )
        .expect("Failed to log container blueprint");

    stream
        .log(
            "viewport",
            &ViewportBlueprint::new().with_root_container(RootContainer(container_id.into())),
        )
        .expect("Failed to log viewport blueprint");

    stream
        .log(
            "table",
            &TableBlueprint::new()
                .with_segment_preview_column(preview_column)
                .with_grid_view_card_title(title_column)
                // Clicking a card opens the recording referenced by this column.
                .with_url_column(preview_column),
        )
        .expect("Failed to log table blueprint");

    file
}
