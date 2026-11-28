//! Tests for how the viewer's time control reacts to commands and URLs.

use re_log_channel::RecordingOpenBehavior;
use re_log_types::RecordingId;
use re_sdk_types::blueprint::components::PlayState;
use re_test_context::TestContext;
use re_viewer_context::TimeControlCommand;
use re_viewer_context::viewer_url::{OpenUrlOptions, ViewerUrl};

#[test]
fn opening_url_with_temporal_anchor_pauses_playing_recording() {
    let test_context = TestContext::new();

    // Create a store.
    let dataset_id = re_log_types::external::re_tuid::Tuid::new();
    let segment_id = RecordingId::random();
    let url: ViewerUrl = format!(
        "rerun+http://localhost:51234/dataset/{dataset_id}?segment_id={segment_id}#when=stable_time@+3.990s"
    )
    .parse()
    .expect("test URL should parse");
    let store_id = match &url {
        ViewerUrl::RedapDataset(uri) => uri
            .store_id()
            .expect("test URL should name a segment of the dataset"),
        _ => unreachable!("test URL should be a dataset segment"),
    };
    test_context.store_hub.lock().entity_db_entry(&store_id);

    // Start playback.
    test_context.send_time_commands(
        store_id.clone(),
        [TimeControlCommand::SetPlayState(PlayState::Playing)],
    );
    test_context.handle_system_commands(&egui::Context::default());
    assert_eq!(
        test_context.time_ctrl.read().play_state(),
        PlayState::Playing
    );

    // Open the URL with a `when=…` fragment to select a time and pause playback.
    test_context.run_once_in_egui_central_panel(|ctx, ui| {
        url.open(
            ui.ctx(),
            &OpenUrlOptions {
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
