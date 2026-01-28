mod app_testing_ext;

#[cfg(feature = "testing")]
pub use app_testing_ext::AppTestingExt;
use egui_kittest::Harness;
use re_build_info::build_info;
use re_viewer_context::AppOptions;

use crate::{
    App, AppEnvironment, AsyncRuntimeHandle, MainThreadToken, StartupOptions,
    customize_eframe_and_setup_renderer,
};

pub type AppOptionsEditor = Box<dyn Fn(&mut AppOptions)>;

#[derive(Default)]
pub struct HarnessOptions {
    pub window_size: Option<egui::Vec2>,
    pub max_steps: Option<u64>,
    pub step_dt: Option<f32>,
    pub startup_url: Option<String>,
    pub edit_app_options: Option<AppOptionsEditor>,
}

/// Convenience function for creating a kittest harness of the viewer App.
pub fn viewer_harness(options: &HarnessOptions) -> Harness<'static, App> {
    let window_size = options.window_size.unwrap_or(egui::vec2(1024.0, 768.0));

    let mut harness_builder =
        re_ui::testing::new_harness(re_ui::testing::TestOptions::Rendering3D, window_size);
    if let Some(max_steps) = options.max_steps {
        harness_builder = harness_builder.with_max_steps(max_steps);
    }
    if let Some(step_dt) = options.step_dt {
        harness_builder = harness_builder.with_step_dt(step_dt);
    }

    harness_builder.build_eframe(|cc| {
        cc.egui_ctx.set_os(egui::os::OperatingSystem::Nix);
        customize_eframe_and_setup_renderer(cc).expect("Failed to customize eframe");
        let mut app = App::new(
            MainThreadToken::i_promise_i_am_only_using_this_for_a_test(),
            build_info!(),
            AppEnvironment::Test,
            StartupOptions {
                // Don't show the welcome / example screen in tests.
                // See also: https://github.com/rerun-io/rerun/issues/10989
                hide_welcome_screen: true,
                // Don't calculate memory limit in tests.
                memory_limit: re_memory::MemoryLimit::UNLIMITED,
                ..Default::default()
            },
            cc,
            Some(re_redap_client::ConnectionRegistry::new_without_stored_credentials()),
            AsyncRuntimeHandle::from_current_tokio_runtime_or_wasmbindgen()
                .expect("Failed to create AsyncRuntimeHandle"),
        );
        // Force the FFmpeg path to be wrong so we have a reproducible behavior.
        app.app_options_mut().video.ffmpeg_path = "/fake/ffmpeg/path".to_owned();
        app.app_options_mut().video.override_ffmpeg_path = true;

        if let Some(edit_app_options) = &options.edit_app_options {
            edit_app_options(app.app_options_mut());
        }
        // This is slightly different than calling this after we created the harness since
        // the harness will do some stepping upon creation.
        // Opening a URL directly after creation is much closer to the behavior when opening URL from command line start.
        if let Some(startup_url) = &options.startup_url {
            app.open_url_or_file(startup_url);
        }

        app
    })
}

/// Steps through the harness until the `predicate` closure returns `true`.
#[track_caller]
pub fn step_until<'app, 'harness, Predicate>(
    test_description: &'static str,
    harness: &'harness mut egui_kittest::Harness<'app, App>,
    mut predicate: Predicate,
    step_duration: std::time::Duration,
    max_duration: std::time::Duration,
) where
    Predicate: for<'a> FnMut(&'a egui_kittest::Harness<'app, App>) -> bool,
{
    let start_time = std::time::Instant::now();
    let mut success = predicate(harness);
    while !success && start_time.elapsed() <= max_duration {
        harness.step();
        std::thread::sleep(step_duration);
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
