//! Tests for undo/redo in the viewer.
//!
//! This test verifies that:
//! - Dragging to rotate a 3D view changes the camera orientation
//! - Undo (Cmd/Ctrl+Z) reverts the camera orientation
//! - Redo restores the undone changes
//! - After undo, performing a new action clears the redo history

use re_integration_test::HarnessExt as _;
use re_sdk::TimePoint;
use re_sdk::log::RowId;
use re_viewer::external::re_ui::{UICommand, UICommandSender as _};
use re_viewer::external::re_viewer_context::ViewClass as _;
use re_viewer::external::{re_sdk_types, re_view_spatial};
use re_viewer::viewer_test_utils::{self, HarnessOptions};
use re_viewport_blueprint::ViewBlueprint;

fn make_undo_test_harness<'a>() -> egui_kittest::Harness<'a, re_viewer::App> {
    let mut harness = viewer_test_utils::viewer_harness(&HarnessOptions {
        window_size: Some(egui::vec2(800.0, 600.0)),
        max_steps: Some(200),      // Allow interactions to complete.
        step_dt: Some(1.0 / 60.0), // 60 FPS simulation.
        ..Default::default()
    });
    harness.init_recording();
    harness.set_selection_panel_opened(false);
    harness.set_blueprint_panel_opened(false);
    harness.set_time_panel_opened(false);

    // Log a 3D box at the origin.
    harness.log_entity("box", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::STATIC,
            &re_sdk_types::archetypes::Boxes3D::from_centers_and_half_sizes(
                [(0.0, 0.0, 0.0)],
                [(0.5, 0.5, 0.5)],
            )
            .with_colors([0xFF0000FF])
            .with_fill_mode(re_sdk_types::components::FillMode::Solid),
        )
    });

    // Clear existing blueprint and create a single 3D view.
    harness.clear_current_blueprint();

    let mut view_3d =
        ViewBlueprint::new_with_root_wildcard(re_view_spatial::SpatialView3D::identifier());
    view_3d.display_name = Some("3D view".into());

    harness.setup_viewport_blueprint(move |_viewer_context, blueprint| {
        blueprint.add_view_at_root(view_3d);
    });

    harness
}

/// Helper to perform a drag rotation on the 3D view.
fn drag_rotate_view(
    harness: &mut egui_kittest::Harness<'_, re_viewer::App>,
    start_offset: egui::Vec2,
    drag_delta: egui::Vec2,
) {
    let view_rect = harness.get_panel_position("3D view");
    let start_pos = view_rect.center() + start_offset;
    let end_pos = start_pos + drag_delta;

    // Start drag
    harness.drag_at(start_pos);

    // Move during drag
    harness.hover_at(end_pos);

    // End drag
    harness.drop_at(end_pos);

    // Let the UI settle after the interaction
    harness.run();
}

/// Send undo command (Cmd/Ctrl+Z).
fn send_undo(harness: &mut egui_kittest::Harness<'_, re_viewer::App>) {
    harness.state().command_sender.send_ui(UICommand::Undo);
    harness.run();
}

/// Send redo command (Cmd/Ctrl+Shift+Z).
fn send_redo(harness: &mut egui_kittest::Harness<'_, re_viewer::App>) {
    harness.state().command_sender.send_ui(UICommand::Redo);
    harness.run();
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_3d_view_rotation_undo_redo() {
    let mut harness = make_undo_test_harness();

    // Initial state - take a snapshot of the initial camera orientation.
    harness.snapshot_app("undo_redo_3d_rotation_1_initial");

    // Drag to rotate the view (rotate right).
    drag_rotate_view(&mut harness, egui::vec2(0.0, 0.0), egui::vec2(100.0, 0.0));
    harness.snapshot_app("undo_redo_3d_rotation_2_after_first_drag");

    // Undo the rotation.
    send_undo(&mut harness);
    harness.snapshot_app("undo_redo_3d_rotation_3_after_undo");

    // Redo the rotation.
    send_redo(&mut harness);
    harness.snapshot_app("undo_redo_3d_rotation_4_after_redo");

    // Undo again.
    send_undo(&mut harness);
    harness.snapshot_app("undo_redo_3d_rotation_5_after_second_undo");

    // Perform a new drag (rotate down instead of right) - this should clear redo history.
    drag_rotate_view(&mut harness, egui::vec2(0.0, 0.0), egui::vec2(0.0, 100.0));
    harness.snapshot_app("undo_redo_3d_rotation_6_after_new_drag");

    // Try to redo - should have no effect since we performed a new action.
    send_redo(&mut harness);
    harness.snapshot_app("undo_redo_3d_rotation_7_redo_after_new_action");
}

#[tokio::test(flavor = "multi_thread")]
pub async fn test_multiple_undo_redo() {
    let mut harness = make_undo_test_harness();

    // Initial state.
    harness.snapshot_app("multiple_undo_redo_1_initial");

    // First rotation (right).
    drag_rotate_view(&mut harness, egui::vec2(0.0, 0.0), egui::vec2(80.0, 0.0));
    harness.snapshot_app("multiple_undo_redo_2_after_drag_1");

    // Second rotation (down).
    drag_rotate_view(&mut harness, egui::vec2(0.0, 0.0), egui::vec2(0.0, 80.0));
    harness.snapshot_app("multiple_undo_redo_3_after_drag_2");

    // Undo second rotation.
    send_undo(&mut harness);
    harness.snapshot_app("multiple_undo_redo_4_undo_once");

    // Undo first rotation.
    send_undo(&mut harness);
    harness.snapshot_app("multiple_undo_redo_5_undo_twice");

    // Redo first rotation.
    send_redo(&mut harness);
    harness.snapshot_app("multiple_undo_redo_6_redo_once");

    // Redo second rotation.
    send_redo(&mut harness);
    harness.snapshot_app("multiple_undo_redo_7_redo_twice");
}
