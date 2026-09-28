//! End-to-end test for table segment previews.
//!
//! Builds remote tables whose rows carry recording URIs and an embedded table blueprint
//! that defines `Spatial3DView`s.
//! The viewer loads the referenced recordings on demand and renders them in preview columns.
//! The tests verify that the recordings load and that the rendered previews match snapshots.
//!
//! The referenced recording logs its `Points3D` statically, so the preview renders the same
//! at every point on its looping preview timeline and the snapshot stays stable.

use std::str::FromStr as _;
use std::sync::Arc;
use std::time::Duration;

use arrow::array::{ArrayRef, Int64Array, RecordBatch, RecordBatchOptions, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use egui_kittest::kittest::Queryable as _;

use re_integration_test::ViewerHarnessExt as _;
use re_integration_test::{HarnessExt as _, TestServer};
use re_sdk::RecordingStreamBuilder;
use re_sdk::external::{re_log_types, re_tuid};
use re_sdk_types::blueprint::archetypes::{
    CardLayout, ContainerBlueprint, TableColumn, TableColumnPreview, TableLayout, ViewBlueprint,
    ViewContents, ViewportBlueprint,
};
use re_sdk_types::blueprint::components::{
    ContainerKind, IncludedContent, QueryExpression, RootContainer, ViewClass,
};
use re_viewer::viewer_test_utils::{self, HarnessOptions};

const DATASET_ID: &str = "187b552b95a5c2f73f37894708825ba5";
const PREVIEW_COLUMN: &str = "recording_uri";
const SECOND_PREVIEW_COLUMN: &str = "recording_uri_2";
const TITLE_COLUMN: &str = "name";
const SEGMENT_COUNT: usize = 4;

#[derive(Clone, Copy)]
struct PreviewColumnSpec {
    name: &'static str,
    num_views: usize,
}

struct PreviewTableFixture {
    _server: TestServer,
    startup_url: String,
    segment_uris: Vec<re_uri::DatasetUri>,
}

#[tokio::test(flavor = "multi_thread")]
pub async fn preview_table() {
    let fixture = preview_table_fixture(&[PreviewColumnSpec {
        name: PREVIEW_COLUMN,
        num_views: 1,
    }])
    .await;

    // Open the viewer directly at the table entry. Make the window tall enough that all rows
    // are on screen at once, so every preview loads.
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::vec2(1024.0, 1000.0)),
        startup_url: Some(fixture.startup_url),
        snapshot_test_options: re_ui::testing::TestOptions::Rendering3D, // Need higher thresholds for the 3D content.
        ..Default::default()
    });
    let segment_uris = fixture.segment_uris;

    // Step until every preview recording has actually streamed in its point data. Rendering the
    // preview column is what triggers the background loads, so this also exercises the column.
    let preview_uris = wait_for_preview_recordings(&mut harness, &segment_uris);

    // Card layout is the default when configured.
    // Let the 3D views' camera framing settle before snapshotting.
    harness.run_ok();
    harness.snapshot("previews_cards");

    // Switch to the table layout and snapshot the same previews as table columns.
    harness.get_by_label("Table view").click();
    harness.run_ok();
    harness.snapshot("previews_table");

    // Return to cards before testing card activation.
    harness.get_by_label("Cards view").click();
    harness.run_ok();

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
    viewer_test_utils::step_until_with_custom_timeout(
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

    viewer_test_utils::step_until_with_custom_timeout(
        "Opened recording source tree populated",
        &mut harness,
        |harness| harness.query_by_label_contains("Streams").is_some(),
        Duration::from_millis(100),
        Duration::from_secs(15),
    );
    harness.step_until_no_loading_indicator();

    // Close the selection panel rather than masking it: it shows the recording URI, which
    // embeds the server's random port.
    harness.set_selection_panel_opened(false);
    harness.mask_dates();
    harness.snapshot("preview_table_opened_recording");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn preview_table_with_multiple_preview_columns() {
    // The first column selects the only view in the viewport, while the second also selects a
    // view outside the viewport. This verifies that each renderer uses its column configuration.
    let fixture = preview_table_fixture(&[
        PreviewColumnSpec {
            name: PREVIEW_COLUMN,
            num_views: 1,
        },
        PreviewColumnSpec {
            name: SECOND_PREVIEW_COLUMN,
            num_views: 2,
        },
    ])
    .await;

    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::vec2(1400.0, 1000.0)),
        startup_url: Some(fixture.startup_url),
        snapshot_test_options: re_ui::testing::TestOptions::Rendering3D,
        ..Default::default()
    });
    let segment_uris = fixture.segment_uris;
    viewer_test_utils::step_until("table layout toggle appears", &mut harness, |harness| {
        harness.query_by_label("Table view").is_some()
    });

    // Two preview fields are stacked vertically in each card. The second field contains two views,
    // which are laid out side by side. The taller cards leave only the first two rows on screen.
    wait_for_preview_recordings(&mut harness, &segment_uris[..2]);
    harness.run_ok();
    harness.snapshot("preview_cards_multiple_preview_columns");

    harness.get_by_label("Table view").click();
    harness.run_ok();
    wait_for_preview_recordings(&mut harness, &segment_uris);
    harness.snapshot("preview_table_multiple_preview_columns");
}

fn wait_for_preview_recordings(
    harness: &mut egui_kittest::Harness<'_, re_viewer::App>,
    segment_uris: &[re_uri::DatasetUri],
) -> Vec<re_uri::DatasetUri> {
    let preview_uris = segment_uris
        .iter()
        .map(|uri| uri.clone().without_fragment())
        .collect::<Vec<_>>();
    let preview_entity = re_log_types::EntityPath::from("test_entity");
    viewer_test_utils::step_until_with_custom_timeout(
        "All preview recordings loaded",
        harness,
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
    preview_uris
}

async fn preview_table_fixture(preview_columns: &[PreviewColumnSpec]) -> PreviewTableFixture {
    let (server, segment_ids) = TestServer::spawn()
        .await
        .with_static_preview_data(
            "preview_dataset",
            DATASET_ID,
            "preview_recording",
            SEGMENT_COUNT,
        )
        .await;

    let dataset_id = re_tuid::Tuid::from_str(DATASET_ID).expect("Failed to parse TUID");
    let segment_uris = segment_ids
        .iter()
        .map(|segment_id| re_uri::DatasetUri {
            origin: re_uri::Origin {
                scheme: re_uri::Scheme::RerunHttp,
                host: re_uri::external::url::Host::Domain("localhost".to_owned()),
                port: server.port(),
            },
            dataset_id,
            resource: re_uri::DatasetResource::Segments,
            segment_id: Some(segment_id.clone()),
            fragment: Default::default(),
        })
        .collect::<Vec<_>>();

    let mut fields = vec![
        Field::new("id", DataType::Int64, false)
            .with_metadata([("rerun:is_table_index".to_owned(), "true".to_owned())].into()),
        Field::new(TITLE_COLUMN, DataType::Utf8, false),
    ];
    fields.extend(
        preview_columns
            .iter()
            .map(|preview| Field::new(preview.name, DataType::Utf8, false)),
    );
    let schema = Arc::new(Schema::new_with_metadata(fields, Default::default()));

    let connection = server.connection_handle();
    let mut client = connection
        .client()
        .await
        .expect("Failed to connect to server");
    let table = client
        .create_table_entry(
            re_log_types::EntryName::new("preview_table").expect("valid entry name"),
            None,
            schema.clone(),
        )
        .await
        .expect("Failed to create table");

    let names = (0..SEGMENT_COUNT)
        .map(|i| format!("segment {i}"))
        .collect::<Vec<_>>();
    let mut columns: Vec<ArrayRef> = vec![
        Arc::new(Int64Array::from_iter_values(
            (0..SEGMENT_COUNT).map(|i| i64::try_from(i).expect("segment index fits in i64")),
        )),
        Arc::new(StringArray::from(names)),
    ];
    let uri_strings = segment_uris
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    columns.extend(
        preview_columns
            .iter()
            .map(|_| Arc::new(StringArray::from(uri_strings.clone())) as ArrayRef),
    );
    let batch = RecordBatch::try_new_with_options(
        schema,
        columns,
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

    let blueprint_rbl = blueprint_rbl_file(preview_columns, TITLE_COLUMN);
    re_integration_test::register_table_blueprint(&connection, &table, blueprint_rbl.path())
        .await
        .expect("Failed to register table blueprint");

    PreviewTableFixture {
        startup_url: format!(
            "rerun+http://localhost:{}/entry/{}",
            server.port(),
            table.details.id
        ),
        _server: server,
        segment_uris,
    }
}

/// Build a `.rbl` blueprint file holding `Spatial3DView`s over `/test_entity` plus
/// table layout archetypes pointing previews at the configured columns and card titles at
/// `title_column`.
///
/// TODO(andreas): Should use a higher level rust blueprint api.
fn blueprint_rbl_file(
    preview_columns: &[PreviewColumnSpec],
    title_column: &str,
) -> tempfile::NamedTempFile {
    let file = tempfile::Builder::new()
        .suffix(".rbl")
        .tempfile()
        .expect("Failed to create blueprint temp file");

    let stream = RecordingStreamBuilder::new("rerun_example_table_blueprint")
        .blueprint()
        .save(file.path())
        .expect("Failed to create blueprint memory stream");
    stream.set_time_sequence("blueprint", 0);

    let num_views = preview_columns
        .iter()
        .map(|preview| preview.num_views)
        .max()
        .expect("At least one preview column is required");
    let view_paths = (0..num_views)
        .map(|_| {
            let view_path = format!("view/{}", uuid::Uuid::new_v4());
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
            view_path
        })
        .collect::<Vec<_>>();

    let container_id = uuid::Uuid::new_v4();
    stream
        .log(
            format!("container/{container_id}"),
            &ContainerBlueprint::new(ContainerKind::Tabs)
                .with_contents([IncludedContent(view_paths[0].clone().into())]),
        )
        .expect("Failed to log container blueprint");
    stream
        .log(
            "viewport",
            &ViewportBlueprint::new().with_root_container(RootContainer(container_id.into())),
        )
        .expect("Failed to log viewport blueprint");

    for preview in preview_columns {
        let views = view_paths
            .iter()
            .take(preview.num_views)
            .cloned()
            .map(|path| IncludedContent(path.into()))
            .collect::<Vec<_>>();
        for base_path in ["table/layouts/table/columns", "table/layouts/cards/fields"] {
            let preview_path = re_log_types::EntityPath::from(base_path)
                / re_log_types::EntityPathPart::new(preview.name);
            stream
                .log(
                    preview_path.clone(),
                    &TableColumn::new().with_cell_kind(
                        re_sdk_types::blueprint::components::TableCellKind::Preview,
                    ),
                )
                .expect("Failed to log preview column blueprint");
            stream
                .log(preview_path, &TableColumnPreview::new(views.clone()))
                .expect("Failed to log preview configuration");
        }
    }

    stream
        .log(
            "table/layouts/table",
            &TableLayout::new()
                .with_column_order(preview_columns.iter().map(|preview| preview.name)),
        )
        .expect("Failed to log table layout");
    stream
        .log(
            "table/layouts/cards",
            &CardLayout::new(preview_columns.iter().map(|preview| preview.name))
                .with_title(title_column)
                .with_link(preview_columns[0].name),
        )
        .expect("Failed to log card layout");

    file
}
