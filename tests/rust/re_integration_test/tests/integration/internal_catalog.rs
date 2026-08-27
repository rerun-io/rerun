//! Testing the internal catalog.
//!
//! As long as we still have the old loading path, we contrast both
//! to highlight things that we still need to adapt.

use std::path::PathBuf;
use std::time::Duration;

use egui_kittest::SnapshotResults;
use egui_kittest::kittest::Queryable as _;
use re_integration_test::HarnessExt as _;
use re_integration_test::ViewerHarnessExt as _;
use re_log_types::{EntityPath, EntryName, TimelineName};
use re_protos::common::v1alpha1::ext::SegmentId;
use re_sdk::RecordingStreamBuilder;
use re_sdk::blueprint::{Blueprint, Spatial2DView};
use re_sdk_types::archetypes::Points2D;
use re_sdk_types::blueprint::archetypes::ViewportBlueprint;
use re_sdk_types::blueprint::components::AutoLayout;
use re_sdk_types::components::{Color, Radius};
use re_sdk_types::encodings::Bool;
use re_viewer::viewer_test_utils::{self, AppTestingExt as _};
use re_viewer_context::Route;

// TODO(RR-4929): We should properly show the application id,
// and maybe even the recording id.

const RRD_RECORDING_ID: &str = "test_recording";
const RRD_APP_ID: &str = "test_app";
const RRD_FILE_NAME: &str = "internal_catalog_test.rrd";
const RBL_FILE_NAME: &str = "internal_catalog_test.rbl";

fn test_blueprint() -> Blueprint {
    Blueprint::new(
        Spatial2DView::new("points")
            .with_origin("/")
            .with_contents(["/points"])
            .with_override(
                "points",
                &Points2D::update_fields().with_colors([Color::from_rgb(0, 255, 0)]),
            ),
    )
}

fn test_recording(path: &std::path::Path) -> re_sdk::RecordingStream {
    let recording = RecordingStreamBuilder::new(RRD_APP_ID)
        .recording_id(RRD_RECORDING_ID)
        // The built-in properties carry the recording start time, which the selection panel would
        // show as an ever-changing timestamp in snapshots.
        .send_properties(false)
        .save(path)
        .expect("failed to create .rrd recording stream");
    recording.set_time_sequence("frame", 0);
    recording
        .log(
            "points",
            &Points2D::new([(0.0, 0.0), (1.0, 1.0)])
                .with_colors([Color::from_rgb(255, 0, 0)])
                .with_radii([Radius::new_ui_points(24.0)]),
        )
        .expect("failed to log points");
    recording
}

fn flush_recording(recording: &re_sdk::RecordingStream, extension: &str) {
    recording
        .flush_with_timeout(Duration::from_mins(1))
        .unwrap_or_else(|err| panic!("failed to flush {extension}: {err}"));
}

fn test_rrd(include_blueprint: bool) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("failed to create .rrd temp dir");
    let path = dir.path().join(RRD_FILE_NAME);
    let recording = test_recording(&path);
    if include_blueprint {
        test_blueprint()
            .send(&recording, Default::default())
            .expect("failed to log blueprint");
    }
    flush_recording(&recording, ".rrd");
    (dir, path)
}

fn test_rbl() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("failed to create .rbl temp dir");
    let path = dir.path().join(RBL_FILE_NAME);

    let stream = RecordingStreamBuilder::new(RRD_APP_ID)
        .blueprint()
        .save(&path)
        .expect("failed to create .rbl blueprint stream");
    stream.set_time_sequence("blueprint", 0);

    // Include a second (ignored) blueprint to verify that all blueprint stores are registered.
    stream
        .log(
            "viewport",
            &ViewportBlueprint::new().with_auto_layout(AutoLayout(Bool(false))),
        )
        .expect("failed to log first blueprint");

    // The last activation determines the default blueprint for the application.
    test_blueprint()
        .send(&stream, Default::default())
        .expect("failed to log last blueprint");

    flush_recording(&stream, ".rbl");

    (dir, path)
}

fn mask_internal_catalog_app_id(harness: &mut egui_kittest::Harness<'_, re_viewer::App>) {
    // TODO(RR-4929): Remove this mask once the catalog app id matches recording app id.
    let app_id = harness
        .state()
        .active_recording_id()
        .map(|store_id| store_id.application_id().to_string())
        .unwrap_or_default();
    let app_id_rects = {
        let selection_panel = harness.selection_panel();
        let selection_panel_root = selection_panel.root();
        let selection_panel_rect = selection_panel_root.rect();
        selection_panel_root
            .query_all_by(|node| {
                node.label().is_some_and(|label| label.contains(&app_id))
                    || node.value().is_some_and(|value| value.contains(&app_id))
            })
            .map(|node| {
                let rect = node.rect();
                egui::Rect::from_min_max(
                    egui::pos2(selection_panel_rect.left(), rect.top()),
                    egui::pos2(selection_panel_rect.right(), rect.bottom()),
                )
            })
            .collect::<Vec<_>>()
    };
    for rect in app_id_rects {
        harness.mask(rect);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn internal_catalog_revealed_by_catalog_api() {
    const DATASET_NAME: &str = "catalog_api_dataset";

    let mut harness = viewer_test_utils::viewer_harness(&Default::default());
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(false);

    assert!(
        harness
            .recording_panel()
            .root()
            .query_by_label_contains("Viewer catalog")
            .is_none()
    );

    let mut client = harness
        .state()
        .connection_registry()
        .internal_connection_handle()
        .expect("internal catalog should be configured")
        .client()
        .await
        .expect("failed to connect to internal catalog");
    client
        .create_dataset_entry(
            EntryName::new(DATASET_NAME).expect("valid entry name"),
            None,
        )
        .await
        .expect("failed to create dataset");

    viewer_test_utils::step_until("internal catalog revealed", &mut harness, |harness| {
        let panel = harness.recording_panel();
        let root = panel.root();
        root.query_by_label_contains("Viewer catalog").is_some()
            && root.query_by_label_contains(DATASET_NAME).is_some()
    });

    harness.snapshot("internal_catalog_revealed_by_catalog_api");
}

#[tokio::test(flavor = "multi_thread")]
async fn internal_catalog_load_rbl() {
    let (_dir, rbl_path) = test_rbl();
    let rbl = std::fs::File::open(&rbl_path).expect("failed to open .rbl");
    let metadata = re_log_encoding::enumerate_legacy_metadata(&rbl)
        .await
        .expect("failed to read .rbl metadata");
    assert!(
        metadata
            .store_ids
            .iter()
            .all(re_log_types::StoreId::is_blueprint)
    );

    let mut expected_segment_ids = metadata
        .store_ids
        .iter()
        .map(|store_id| SegmentId::from(store_id.recording_id()))
        .collect::<Vec<_>>();
    let expected_default_segment = metadata
        .default_blueprint_by_app_id
        .values()
        .next()
        .map(|store_id| SegmentId::from(store_id.recording_id()))
        .expect(".rbl should contain a default blueprint activation");

    let mut harness = viewer_test_utils::viewer_harness(&viewer_test_utils::HarnessOptions {
        app_options_editor: Some(Box::new(|app_options| {
            app_options.experimental.use_viewer_catalog = true;
        })),
        ..Default::default()
    });
    harness
        .state()
        .open_url_or_file(&rbl_path.display().to_string());

    viewer_test_utils::step_until("blueprint catalog entry opened", &mut harness, |harness| {
        matches!(
            harness.state().testonly_get_route(),
            Route::RedapEntry { .. }
        )
    });

    let Route::RedapEntry {
        origin, entry_id, ..
    } = harness.state().testonly_get_route()
    else {
        panic!("expected a catalog entry route");
    };
    let origin = origin.clone();
    let entry_id = *entry_id;
    let connection_registry = harness.state().connection_registry().clone();
    let mut client = connection_registry
        .connection_handle(origin)
        .client()
        .await
        .expect("failed to connect to internal catalog");
    let dataset = client
        .read_dataset_entry(entry_id)
        .await
        .expect("failed to read blueprint parent dataset");
    let blueprint_dataset_id = dataset
        .dataset_details
        .blueprint_dataset
        .expect("parent dataset should have a blueprint dataset");
    assert_eq!(
        dataset.dataset_details.default_blueprint_segment,
        Some(expected_default_segment)
    );

    let mut actual_segment_ids = client
        .get_dataset_segment_ids(blueprint_dataset_id)
        .await
        .expect("failed to list registered blueprints");

    expected_segment_ids.sort_by_key(ToString::to_string);
    actual_segment_ids.sort_by_key(ToString::to_string);
    assert_eq!(actual_segment_ids, expected_segment_ids);

    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(false);

    // The blueprint-only entry appears as an empty dataset.
    harness.step_until_no_loading_indicator();
    harness.snapshot("internal_catalog_load_rbl_1");

    let (_recording_dir, recording_path) = test_rrd(false);
    harness
        .state()
        .open_url_or_file(&recording_path.display().to_string());

    let points = EntityPath::from("points");
    let frame = TimelineName::from("frame");
    viewer_test_utils::step_until(
        "recording loaded with registered blueprint",
        &mut harness,
        move |harness| {
            let Some(store_id) = harness.state().active_recording_id().cloned() else {
                return false;
            };
            if store_id.recording_id().as_str() != RRD_RECORDING_ID {
                return false;
            }

            let points = points.clone();
            harness.run_with_app_context(move |ctx| {
                ctx.storage_context
                    .hub
                    .entity_db(&store_id)
                    .is_some_and(|db| {
                        db.data_source
                            .as_ref()
                            .is_some_and(|source| source.is_redap())
                            && db
                                .storage_engine()
                                .store()
                                .entity_has_physical_temporal_data_on_timeline(&points, &frame)
                    })
            })
        },
    );

    harness.set_blueprint_panel_opened(true);
    harness.set_selection_panel_opened(false);
    harness.set_time_panel_opened(false);

    // The recording opens with the registered blueprint applied, showing two green points.
    harness.snapshot("internal_catalog_load_rbl_2");
}

#[tokio::test(flavor = "multi_thread")]
async fn internal_catalog_load_rrd() {
    let mut snapshot_results = SnapshotResults::new();

    fn run_with_catalog(snapshot_results: &mut SnapshotResults, use_viewer_catalog: bool) {
        let (dir, rrd_path) = test_rrd(true);
        let mut harness = viewer_test_utils::viewer_harness(&viewer_test_utils::HarnessOptions {
            app_options_editor: Some(Box::new(move |app_options| {
                app_options.experimental.use_viewer_catalog = use_viewer_catalog;
            })),
            ..Default::default()
        });

        harness
            .state()
            .open_url_or_file(&rrd_path.display().to_string());

        let points = EntityPath::from("points");
        let frame = TimelineName::from("frame");
        viewer_test_utils::step_until("file loaded", &mut harness, move |harness| {
            let Some(store_id) = harness.state().active_recording_id().cloned() else {
                return false;
            };
            if store_id.recording_id().as_str() != RRD_RECORDING_ID {
                return false;
            }

            let points = points.clone();
            harness.run_with_app_context(move |ctx| {
                ctx.storage_context
                    .hub
                    .entity_db(&store_id)
                    .is_some_and(|db| {
                        db.data_source
                            .as_ref()
                            .is_some_and(|source| source.is_redap() == use_viewer_catalog)
                            && db
                                .storage_engine()
                                .store()
                                .entity_has_physical_temporal_data_on_timeline(&points, &frame)
                    })
            })
        });

        let loading_rrd_toast = format!("Loading {rrd_path:?}…");
        viewer_test_utils::step_until("loading toast gone", &mut harness, |harness| {
            harness
                .query_all_by_label_contains(&loading_rrd_toast)
                .next()
                .is_none()
        });

        harness.set_time_panel_opened(false);

        if use_viewer_catalog {
            mask_internal_catalog_app_id(&mut harness);
        }

        if !use_viewer_catalog {
            // Mask the unstable temp-dir path wherever it appears.
            let temp_dir_path = dir.path().display().to_string();
            let unstable_path_rects: Vec<egui::Rect> = harness
                .query_all_by(|node| {
                    node.label().is_some_and(|l| l.contains(&temp_dir_path))
                        || node.value().is_some_and(|v| v.contains(&temp_dir_path))
                })
                .map(|node| node.rect())
                .collect();
            for rect in unstable_path_rects {
                harness.mask(rect);
            }

            let selection_panel_path_rects = {
                let selection_panel = harness.selection_panel();
                let selection_panel_root = selection_panel.root();
                let selection_panel_rect = selection_panel_root.rect();
                selection_panel_root
                    .query_all_by(|node| {
                        node.label().is_some_and(|l| l.contains(&temp_dir_path))
                            || node.value().is_some_and(|v| v.contains(&temp_dir_path))
                    })
                    .map(|node| {
                        let rect = node.rect();
                        egui::Rect::from_min_max(
                            egui::pos2(selection_panel_rect.left(), rect.top()),
                            egui::pos2(selection_panel_rect.right(), rect.bottom()),
                        )
                    })
                    .collect::<Vec<_>>()
            };
            for rect in selection_panel_path_rects {
                harness.mask(rect);
            }
        }

        let suffix = if use_viewer_catalog {
            "catalog"
        } else {
            "recording"
        };

        harness.snapshot(format!("internal_catalog_load_rrd_{suffix}"));
        snapshot_results.extend_harness(&mut harness);
    }

    run_with_catalog(&mut snapshot_results, true);
    run_with_catalog(&mut snapshot_results, false);
}
