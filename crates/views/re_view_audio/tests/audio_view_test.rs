use egui::accesskit::Role;
use re_chunk::{RowId, TimePoint, Timeline};
use re_log_types::TimeReal;
use re_sdk_types::archetypes::AssetAudio;
use re_sdk_types::blueprint::components::PlayState;
use re_sdk_types::components::MediaType;
use re_test_context::TestContext;
use re_test_context::external::egui_kittest::kittest::Queryable as _;
use re_test_viewport::TestContextExt as _;
use re_view_audio::AudioView;
use re_viewer_context::{TimeControlCommand, ViewClass as _, ViewId};
use re_viewport_blueprint::ViewBlueprint;

fn setup_blueprint(test_context: &mut TestContext) -> ViewId {
    test_context.setup_viewport_blueprint(|_ctx, blueprint| {
        blueprint.add_view_at_root(ViewBlueprint::new_with_root_wildcard(
            AudioView::identifier(),
        ))
    })
}

/// Decoding the test song takes well under a second; this only guards against a hang.
const DECODE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// Decoding runs on a background thread, so the view shows a loading indicator until it is done.
///
/// Steps a throwaway harness until the indicator is gone, so the snapshot harness can settle.
fn wait_for_decoding(test_context: &TestContext, view_id: ViewId) {
    let mut harness = test_context
        .setup_kittest_for_rendering_ui(egui::vec2(400.0, 180.0))
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));

    let start = std::time::Instant::now();
    while start.elapsed() < DECODE_TIMEOUT {
        harness.step();
        if harness.query_by_role(Role::ProgressIndicator).is_none() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("audio did not finish decoding within {DECODE_TIMEOUT:?}");
}

fn setup_song(play_state: PlayState) -> (TestContext, ViewId) {
    // Read at runtime rather than through `env!`, since CI runs the tests from an archive built at
    // another path and remaps this variable to where they run.
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").map_or_else(
        || std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        std::path::PathBuf::from,
    );
    let song = std::fs::read(manifest_dir.join("../../../tests/assets/audio/toreador_song.aac"))
        .expect("the song asset should exist (is git-lfs installed?)");

    let mut test_context = TestContext::new_with_view_class::<AudioView>();

    let timeline = Timeline::new_duration("time");
    test_context.log_entity("song", |builder| {
        builder.with_archetype(
            RowId::new(),
            TimePoint::from([(timeline, 0)]),
            &AssetAudio::from_file_contents(song.clone(), Some(MediaType::aac())),
        )
    });

    test_context.set_active_timeline(*timeline.name());
    test_context.set_time(TimeReal::from(60.0e9));
    test_context.send_time_commands(
        test_context.active_store_id(),
        [TimeControlCommand::SetPlayState(play_state)],
    );
    test_context.handle_system_commands(&egui::Context::default());

    let view_id = setup_blueprint(&mut test_context);
    wait_for_decoding(&test_context, view_id);

    (test_context, view_id)
}

#[test]
fn test_audio_view_requests_playback_only_while_time_advances() {
    let (test_context, view_id) = setup_song(PlayState::Playing);

    // Decoding ran frames at a fixed time, which reads as a held time cursor.
    test_context.set_time(TimeReal::from(61.0e9));
    let mut playback_harness = test_context
        .setup_kittest_for_rendering_ui(egui::vec2(600.0, 180.0))
        .build_ui(|ui| test_context.run_with_single_view(ui, view_id));
    playback_harness.step();
    let num_requests = test_context.num_audio_requests();
    assert!(
        0 < num_requests,
        "a playing view should request audio playback"
    );

    playback_harness.step();
    assert_eq!(
        test_context.num_audio_requests(),
        num_requests,
        "a held time cursor should silence playback instead of looping a slice of audio"
    );
}

/// A real recording, to check the waveform rendering and zoom on something that is not a sine.
#[test]
fn test_audio_view_song() {
    let (test_context, view_id) = setup_song(PlayState::Paused);

    test_context
        .run_view_ui_and_save_snapshot(view_id, "audio_view_song", egui::vec2(600.0, 180.0), None)
        .unwrap();
}
