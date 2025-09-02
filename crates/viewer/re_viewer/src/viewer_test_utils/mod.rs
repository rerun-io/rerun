use egui_kittest::Harness;
use re_build_info::build_info;

use crate::{
    App, AppEnvironment, AsyncRuntimeHandle, MainThreadToken, StartupOptions,
    customize_eframe_and_setup_renderer,
};

/// Convenience function for creating a kittest harness of the viewer App.
pub fn viewer_harness() -> Harness<'static, App> {
    Harness::builder()
        .wgpu()
        .with_size(egui::vec2(1500., 1000.))
        .build_eframe(|cc| {
            cc.egui_ctx.set_os(egui::os::OperatingSystem::Nix);
            customize_eframe_and_setup_renderer(cc).expect("Failed to customize eframe");
            let mut app = App::new(
                MainThreadToken::i_promise_i_am_only_using_this_for_a_test(),
                build_info!(),
                AppEnvironment::Test,
                StartupOptions::default(),
                cc,
                None,
                AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()
                    .expect("Failed to create AsyncRuntimeHandle"),
            );
            // Force the FFmpeg path to be wrong so we have a reproducible behavior.
            app.app_options_mut().video_decoder_ffmpeg_path = "/fake/ffmpeg/path".to_owned();
            app.app_options_mut().video_decoder_override_ffmpeg_path = true;
            app
        })
}

/// Steps through the harness until the `predicate` closure returns `true`.
pub async fn step_until<'app, 'harness, Predicate>(
    test_description: &'static str,
    harness: &'harness mut egui_kittest::Harness<'app, App>,
    mut predicate: Predicate,
    step_duration: tokio::time::Duration,
    max_duration: tokio::time::Duration,
) where
    Predicate: for<'a> FnMut(&'a egui_kittest::Harness<'app, App>) -> bool,
{
    let start_time = std::time::Instant::now();
    let mut success = predicate(harness);
    while !success && start_time.elapsed() <= max_duration {
        harness.step();
        tokio::time::sleep(step_duration).await;
        harness.step();
        success = predicate(harness);
    }

    if !success {
        // Take a screenshot of the state of the harness if we failed the test.
        // This is invaluable for debugging test failures.
        let snapshot_path = "tests/failures";
        harness
            .try_snapshot_options(
                test_description,
                &egui_kittest::SnapshotOptions::default().output_path(snapshot_path),
            )
            .ok();

        panic!(
            "Timed out waiting for predicate to be true for {test_description:?}. A screenshot of the harness has been saved to `{snapshot_path}/{test_description}.new.png`."
        );
    }
}
