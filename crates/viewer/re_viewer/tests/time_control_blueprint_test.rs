//! Verifies that `TimeControl::from_blueprint_with_fallback_play_state` respects
//! a `PlayState` set in the blueprint and only applies the fallback when the
//! blueprint did not specify one.
//!
//! Regression test for <https://github.com/rerun-io/rerun/issues/12773>.

use re_log_channel::RecordingOpenBehavior;
use re_log_types::{RecordingId, TimeReal};
use re_sdk_types::blueprint::components::PlayState;
use re_test_context::TestContext;
use re_viewer_context::open_url::{OpenUrlOptions, ViewerOpenUrl};
use re_viewer_context::{TimeControl, TimeControlCommand};

#[test]
fn empty_blueprint_applies_fallback_play_state() {
    let test_context = TestContext::new();

    let result = test_context.with_blueprint_ctx(|blueprint_ctx, _| {
        TimeControl::from_blueprint_with_fallback_play_state(
            &blueprint_ctx,
            None,
            PlayState::Playing,
        )
        .play_state()
    });

    assert_eq!(result, PlayState::Playing);
}

#[test]
fn blueprint_play_state_overrides_fallback() {
    let test_context = TestContext::new();

    // Pin `PlayState::Paused` into the blueprint via the normal command path.
    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetPlayState(PlayState::Paused)],
    );
    test_context.handle_system_commands(&egui::Context::default());

    let result = test_context.with_blueprint_ctx(|blueprint_ctx, _| {
        // Fallback says `Playing`, but the blueprint already has `Paused` — it
        // should win.
        TimeControl::from_blueprint_with_fallback_play_state(
            &blueprint_ctx,
            None,
            PlayState::Playing,
        )
        .play_state()
    });

    assert_eq!(result, PlayState::Paused);
}

#[test]
fn opening_url_with_temporal_anchor_pauses_playing_recording() {
    let test_context = TestContext::new();

    // Create a store.
    let dataset_id = re_log_types::external::re_tuid::Tuid::new();
    let segment_id = RecordingId::random();
    let url: ViewerOpenUrl = format!(
        "rerun+http://localhost:51234/dataset/{dataset_id}?segment_id={segment_id}#when=stable_time@+3.990s"
    )
    .parse()
    .expect("test URL should parse");
    let store_id = match &url {
        ViewerOpenUrl::RedapDatasetSegment(uri) => uri.store_id(),
        _ => unreachable!("test URL should be a dataset segment"),
    };
    test_context.store_hub.lock().entity_db_entry(&store_id);

    // Set things to playing.
    test_context.send_time_commands(
        store_id.clone(),
        [TimeControlCommand::SetPlayState(PlayState::Playing)],
    );
    test_context.handle_system_commands(&egui::Context::default());
    assert_eq!(
        test_context.time_ctrl.read().play_state(),
        PlayState::Playing
    );

    // Open the URL with a `when=…` fragment, which should pin the time to a specific point and pause playback.
    test_context.run_once_in_egui_central_panel(|ctx, ui| {
        url.open(
            ui.ctx(),
            &OpenUrlOptions {
                follow: false,
                recording_open_behavior: RecordingOpenBehavior::OpenAndSelect,
                show_loader: false,
            },
            ctx.command_sender(),
        );
    });
    test_context.handle_system_commands(&egui::Context::default());

    assert_eq!(
        test_context.time_ctrl.read().play_state(),
        PlayState::Paused,
        "opening a URL with a `when=…` fragment should pause an already-playing recording"
    );
}

/// Regression test for the cursor-drag-resumes-playback symptom of rerun#12773.
///
/// `TimeControl::default()` starts with `following: true`. When the blueprint pins
/// `PlayState::Paused`, the resulting state must clear `following` — otherwise a
/// subsequent `SetTime` (cursor drag) routes through `exit_follow_mode`, which
/// flips the state to `Playing` and clobbers the blueprint.
#[test]
fn dragging_cursor_does_not_resume_playback_after_blueprint_pause() {
    let test_context = TestContext::new();
    let store_id = test_context.active_store_id();

    test_context.send_time_commands(
        store_id.clone(),
        [TimeControlCommand::SetPlayState(PlayState::Paused)],
    );
    test_context.handle_system_commands(&egui::Context::default());
    assert_eq!(
        test_context.time_ctrl.read().play_state(),
        PlayState::Paused
    );

    // Simulate dragging the time cursor; the specific target time doesn't matter.
    let drag_target = TimeReal::from(5_i64);
    test_context.send_time_commands(store_id, [TimeControlCommand::SetTimeClamped(drag_target)]);
    test_context.handle_system_commands(&egui::Context::default());

    assert_eq!(
        test_context.time_ctrl.read().play_state(),
        PlayState::Paused,
        "dragging the cursor must not resume playback when paused via blueprint"
    );
}
