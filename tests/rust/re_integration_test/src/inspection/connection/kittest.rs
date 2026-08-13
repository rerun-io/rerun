//! Drive an in-process viewer running inside an [`egui_kittest::Harness`].

use std::sync::Arc;

use egui_inspection::InspectionPlugin;
use egui_inspection::protocol::{Request, Response};
use parking_lot::Mutex;
use re_viewer::App;
use re_viewer::viewer_test_utils::{HarnessOptions, viewer_harness};

/// Upper bound on frames to step the in-process viewer while waiting for a single inspection
/// reply. A safety net against a request that never gets serviced.
const MAX_IN_PROCESS_STEPS: usize = 1024;

/// An in-process viewer, running in an [`egui_kittest::Harness`] and driven through an
/// [`InspectionPlugin`] registered on its context — servicing the same inspection protocol as the
/// real viewer, just synchronously by stepping the harness.
pub(in crate::inspection) struct InProcessConnection {
    harness: egui_kittest::Harness<'static, App>,

    /// The viewer creates an `AsyncRuntimeHandle` (via `Handle::current`) and spawns tasks for data
    /// loading, so it needs an ambient tokio runtime for its whole lifetime. When the test provides
    /// one (a `#[tokio::test]`), we reuse it. Otherwise (a plain `#[test]`) we own one here and
    /// enter it around every step. Its worker threads keep the viewer's spawned tasks progressing
    /// between steps.
    runtime: Option<tokio::runtime::Runtime>,
}

impl InProcessConnection {
    pub(super) fn new(startup_url: Option<String>) -> Self {
        let runtime = if tokio::runtime::Handle::try_current().is_err() {
            Some(
                // We only get here when no ambient runtime exists, and the viewer takes its
                // `AsyncRuntimeHandle` from `Handle::current`, so this harness has to own one on
                // the test's behalf.
                tokio::runtime::Builder::new_multi_thread() // NOLINT: owned by the test harness
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .expect("Failed to build the in-process viewer runtime"),
            )
        } else {
            None
        };

        let guard = runtime.as_ref().map(tokio::runtime::Runtime::enter);
        let harness = viewer_harness(&HarnessOptions {
            startup_url,
            ..Default::default()
        });
        if harness.ctx.plugin_opt::<InspectionPlugin>().is_none() {
            harness.ctx.add_plugin(InspectionPlugin::new(Some(
                "rerun viewer (in-process)".to_owned(),
            )));
        }
        drop(guard);
        Self { harness, runtime }
    }

    /// Enter the owned runtime (if any) so `Handle::current` works while we step the viewer. A
    /// no-op when the test already provides an ambient runtime.
    fn enter(&self) -> Option<tokio::runtime::EnterGuard<'_>> {
        self.runtime.as_ref().map(tokio::runtime::Runtime::enter)
    }

    /// Submit a request to the plugin and step the harness until it replies, panicking on a
    /// [`Response::Error`] reply.
    pub(super) fn request(&mut self, request: Request) -> Response {
        let _guard = self.enter();
        let reply = Arc::new(Mutex::new(None));
        let sink = Arc::clone(&reply);
        self.harness
            .ctx
            .with_plugin::<InspectionPlugin, _>(move |plugin| {
                plugin.submit(request, move |response| {
                    *sink.lock() = Some(response);
                });
            });
        self.harness.ctx.request_repaint();

        for _ in 0..MAX_IN_PROCESS_STEPS {
            self.harness.step();

            if let Some(response) = reply.lock().take() {
                return match response {
                    Response::Error { message } => panic!("Viewer returned an error: {message}"),
                    response => response,
                };
            }
        }
        panic!(
            "In-process viewer did not answer an inspection request within \
             {MAX_IN_PROCESS_STEPS} steps"
        );
    }
}
