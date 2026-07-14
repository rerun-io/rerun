//! Visual regression tests for blueprint view snippets.
//!
//! `docs/snippets/compare_snippet_output.py` verifies that Rust and Python snippets
//! produce the same `.rrd` contents. This test loads the checked-in snippet assets
//! through the viewer's normal file-open path and snapshots the resulting viewport,
//! so blueprint view snippets also have a pixel-level regression gate.

use std::path::Path;
use std::time::Duration;

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;
use egui_kittest::{SnapshotOptions, SnapshotResults};
use re_integration_test::HarnessExt as _;
use re_viewer::external::re_log_types::TimelineName;
use re_viewer::viewer_test_utils::{self, HarnessOptions, step_until};
use re_viewer_context::TimeControlCommand;

const BLUEPRINT_VIEW_SNIPPETS: &[&str] = &[
    "bar_chart",
    "dataframe",
    "graph",
    "map",
    "spatial2d",
    "spatial3d",
    "state_timeline",
    "tensor",
    "text_document",
    "text_log",
    "timeseries",
];

const MAP_VIEW_SNIPPETS: &[&str] = &["map"];

const TIME_BAR_MASK_HEIGHT: f32 = 22.0;

const TEXT_DOCUMENT_IMAGE_MASK: egui::Rect =
    egui::Rect::from_min_max(egui::pos2(0.0, 660.0), egui::pos2(700.0, 740.0));

#[tokio::test]
async fn snippet_blueprint_views_visual_regression() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let snippet_assets_dir = workspace_root.join("tests/assets/rrd/snippets/views");

    let mut results = SnapshotResults::new();

    for snippet_name in BLUEPRINT_VIEW_SNIPPETS {
        let rrd_path = snippet_assets_dir.join(format!("{snippet_name}.rrd"));
        assert!(
            rrd_path.exists(),
            "Missing snippet asset {rrd_path:?}. Run docs/snippets/compare_snippet_output.py --write-missing-backward-assets to create it."
        );

        let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
            window_size: Some(egui::vec2(1024.0, 768.0)),
            startup_url: Some(rrd_path.canonicalize().unwrap().display().to_string()),
            max_steps: Some(300),
            ..Default::default()
        });

        step_until(
            "snippet blueprint view loaded",
            &mut harness,
            |harness| {
                !harness
                    .query_all_by_role(Role::Window)
                    .any(|window| window.query_by_label_contains("Loading").is_some())
                    && harness.state().active_recording_id().is_some()
            },
            Duration::from_millis(100),
            Duration::from_secs(10),
        );

        let on_log_time = harness.run_with_app_context(|ctx| {
            ctx.send_time_commands_to_active_recording(vec![
                TimeControlCommand::Pause,
                TimeControlCommand::MoveEnd,
            ]);
            ctx.active_time_ctrl()
                .is_some_and(|time_ctrl| *time_ctrl.timeline_name() == TimelineName::log_time())
        });
        harness.run_steps(5);

        harness.set_blueprint_panel_opened(false);
        harness.set_selection_panel_opened(false);
        harness.set_time_panel_opened(false);

        if MAP_VIEW_SNIPPETS.contains(snippet_name) {
            let map_rect = harness.get_by_role_and_label(Role::Pane, "MapView").rect();
            harness.mask(map_rect);
        }

        if *snippet_name == "text_document" {
            // The embedded image in the Markdown snippet can be sampled before
            // its final texture contents settle. Keep the textual Markdown UI
            // under test while masking the noisy image strip.
            harness.mask(TEXT_DOCUMENT_IMAGE_MASK);
        }

        if on_log_time {
            let screen = harness.ctx.content_rect();
            let time_bar = egui::Rect::from_min_max(
                egui::pos2(screen.left(), screen.bottom() - TIME_BAR_MASK_HEIGHT),
                screen.max,
            );
            harness.mask(time_bar);
        }

        harness.mask_dates();

        results.add(
            harness.try_snapshot_options(
                format!("snippet_blueprint_view_{snippet_name}"),
                &SnapshotOptions::new()
                    .threshold(2.0)
                    .failed_pixel_count_threshold(50),
            ),
        );
        results.extend_harness(&mut harness);
    }
}
