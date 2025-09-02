use egui_kittest::Harness;
use egui_kittest::kittest::{By, Queryable};
use re_build_info::build_info;
use std::sync::Arc;

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

/// Utility to wait until some widget appears.
pub struct StepUntil {
    step_duration: tokio::time::Duration,
    max_duration: tokio::time::Duration,
    debug_label: String,
}

impl StepUntil {
    pub fn new(debug_label: impl ToString) -> Self {
        Self {
            debug_label: debug_label.to_string(),
            step_duration: tokio::time::Duration::from_millis(100),
            max_duration: tokio::time::Duration::from_secs(5),
        }
    }

    /// Step duration.
    ///
    /// Default is 100ms.
    pub fn step_duration(mut self, duration: tokio::time::Duration) -> Self {
        self.step_duration = duration;
        self
    }

    /// Max duration to wait for the predicate to be true.
    ///
    /// Default is 5 seconds.
    pub fn max_duration(mut self, duration: tokio::time::Duration) -> Self {
        self.max_duration = duration;
        self
    }

    /// Set the max_duration to step_duration * steps
    pub fn steps(mut self, steps: u32) -> Self {
        self.max_duration = self.step_duration * steps;
        self
    }

    /// Step until the predicate returns Some.
    pub async fn run<'app, 'harness: 'pre, 'pre, State, Predicate, R>(
        self,
        harness: &'harness mut egui_kittest::Harness<'app, State>,
        mut predicate: Predicate,
    ) -> R
    where
        Predicate: FnMut(&'pre Harness<'app, State>) -> Option<R>,
    {
        let start_time = std::time::Instant::now();
        loop {
            match predicate(harness) {
                Some(result) => {
                    return result;
                }
                None => {
                    if start_time.elapsed() > self.max_duration {
                        panic!(
                            r#"Timed out waiting for "{}"

Found nodes: {:?}"#,
                            self.debug_label,
                            harness.root()
                        );
                    }
                    tokio::time::sleep(self.step_duration).await;
                    harness.step();
                }
            }
        }
    }
}
