//! Testing the internal catalog.
//!
//! As long as we still have the old loading path, we contrast both
//! to highlight things that we still need to adapt.

use std::path::PathBuf;
use std::time::Duration;

use egui_kittest::SnapshotResults;
use egui_kittest::kittest::Queryable as _;
use re_integration_test::HarnessExt as _;
use re_log_types::{EntityPath, TimelineName};
use re_sdk::RecordingStreamBuilder;
use re_sdk::blueprint::{Blueprint, Spatial2DView};
use re_sdk_types::archetypes::Points2D;
use re_sdk_types::components::{Color, Radius};
use re_viewer::viewer_test_utils;

// TODO(RR-4929): We should properly show the application id,
// and maybe even the recording id.

const RRD_RECORDING_ID: &str = "test_recording";
const RRD_APP_ID: &str = "test_app";
const RRD_FILE_NAME: &str = "internal_catalog_test.rrd";

fn test_rrd() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("failed to create .rrd temp dir");
    let path = dir.path().join(RRD_FILE_NAME);

    let rec = RecordingStreamBuilder::new(RRD_APP_ID)
        .recording_id(RRD_RECORDING_ID)
        .save(&path)
        .expect("failed to create .rrd recording stream");
    rec.set_time_sequence("frame", 0);
    rec.log(
        "points",
        &Points2D::new([(0.0, 0.0), (1.0, 1.0)])
            .with_colors([Color::from_rgb(255, 0, 0)])
            .with_radii([Radius::new_ui_points(24.0)]),
    )
    .expect("failed to log points");

    // TODO(RR-5030): We don't load the blueprint yet, which is why the snapshots differ.
    Blueprint::new(
        Spatial2DView::new("points")
            .with_origin("/")
            .with_contents(["/points"])
            .with_override(
                "points",
                &Points2D::update_fields().with_colors([Color::from_rgb(0, 255, 0)]),
            ),
    )
    .send(&rec, Default::default())
    .expect("failed to log blueprint");

    rec.flush_with_timeout(Duration::from_secs(60))
        .expect("failed to flush .rrd");

    (dir, path)
}

#[tokio::test(flavor = "multi_thread")]
async fn internal_catalog_load_rrd() {
    let mut snapshot_results = SnapshotResults::new();

    fn run_with_catalog(snapshot_results: &mut SnapshotResults, use_internal_catalog: bool) {
        let (dir, rrd_path) = test_rrd();
        let mut harness = viewer_test_utils::viewer_harness(&viewer_test_utils::HarnessOptions {
            app_options_editor: Some(Box::new(move |app_options| {
                app_options.experimental.use_internal_catalog = use_internal_catalog;
            })),
            ..Default::default()
        });

        harness
            .state()
            .open_url_or_file(&rrd_path.display().to_string());

        let points = EntityPath::from("points");
        let frame = TimelineName::from("frame");
        viewer_test_utils::step_until(
            "file loaded",
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
                                .is_some_and(|source| source.is_redap() == use_internal_catalog)
                                && db
                                    .storage_engine()
                                    .store()
                                    .entity_has_physical_temporal_data_on_timeline(&points, &frame)
                        })
                })
            },
            Duration::from_millis(100),
            Duration::from_secs(10),
        );

        let loading_rrd_toast = format!("Loading {rrd_path:?}…");
        viewer_test_utils::step_until(
            "loading toast gone",
            &mut harness,
            |harness| {
                harness
                    .query_by_label_contains(&loading_rrd_toast)
                    .is_none()
            },
            Duration::from_millis(100),
            Duration::from_secs(10),
        );

        harness.set_time_panel_opened(false);

        if use_internal_catalog {
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

        if !use_internal_catalog {
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

        let suffix = if use_internal_catalog {
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
