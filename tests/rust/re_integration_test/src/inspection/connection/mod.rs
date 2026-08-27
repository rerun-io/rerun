//! Transports that speak the `egui_inspection` protocol to a viewer.
//!
//! Each submodule implements one transport; [`Connection`] dispatches between them and turns the
//! raw [`Request`]/[`Response`] pairs into the typed operations the harness needs.

#[cfg(feature = "browser")]
mod browser;
mod grpc;
mod kittest;

use std::time::{Duration, Instant};

use egui::accesskit::TreeUpdate;
use egui_inspection::protocol::{Request, Response};

#[cfg(feature = "browser")]
use browser::BrowserConnection;
use grpc::GrpcConnection;
use kittest::InProcessConnection;

use super::{HarnessConfig, TestEnv};

/// How long to keep retrying the initial connection to the spawned viewer.
pub(super) const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How many frames we expect a viewer to need before it goes quiet.
pub(super) const SETTLE_MAX_STEPS: u64 = 4;

/// How far [`InspectionHarness::run`](crate::InspectionHarness::run) keeps stepping once
/// [`SETTLE_MAX_STEPS`] is exceeded, so it can report the number a test would actually need
/// instead of only that the budget was blown.
pub(super) const SETTLE_DIAGNOSTIC_MAX_STEPS: u64 = 100;

/// A transport to a viewer that speaks the `egui_inspection` protocol.
pub(super) enum Connection {
    /// An out-of-process viewer, driven over the gRPC `Inspect` RPC.
    Grpc(GrpcConnection),

    /// An in-process viewer stepped via [`egui_kittest`], driven through its
    /// [`InspectionPlugin`](egui_inspection::InspectionPlugin).
    InProcess(Box<InProcessConnection>),

    /// The real wasm web viewer running in a browser, driven by calling inspection requests via
    /// chrome dev tools protocol.
    #[cfg(feature = "browser")]
    Browser(Box<BrowserConnection>),
}

impl Connection {
    /// Run the viewer in-process via an [`egui_kittest::Harness`], driven over the inspection
    /// protocol. No subprocess or prebuilt binary needed.
    pub(super) fn spawn_in_process(config: HarnessConfig) -> Self {
        Self::InProcess(Box::new(InProcessConnection::new(config.startup_url)))
    }

    /// Launch a native `rerun --integration-test` process and connect to it over gRPC.
    pub(super) fn spawn_cli(config: &HarnessConfig) -> Self {
        Self::Grpc(GrpcConnection::spawn_viewer(config))
    }

    /// Run the real wasm web viewer in a browser and connect to it by calling its `inspect` method.
    #[cfg(feature = "browser")]
    pub(super) fn spawn_browser(config: &HarnessConfig) -> Self {
        Self::Browser(Box::new(BrowserConnection::new(
            config.size(),
            config.startup_url.as_deref(),
        )))
    }

    /// Send one request and return its response, panicking on transport failure or a
    /// [`Response::Error`] reply.
    fn request(&mut self, request: Request) -> Response {
        let response = match self {
            Self::Grpc(connection) => connection.request(request),
            Self::InProcess(connection) => connection.request(request),
            #[cfg(feature = "browser")]
            Self::Browser(connection) => connection.request(&request),
        };
        // Optionally slow the test down so a developer watching a windowed viewer can follow along.
        if let Some(delay) = TestEnv::get().command_delay {
            std::thread::sleep(delay);
        }
        response
    }

    /// Run frames until the viewer stops asking for immediate repaints, or we exceed `max_steps`.
    ///
    /// Returns `Some(steps)` when we settle, or `None`, if we failed to settle.
    pub(super) fn settle(&mut self, max_steps: u64) -> Option<u64> {
        match self.request(Request::Settle { max_steps }) {
            Response::Settled { settled, steps } => {
                if settled {
                    Some(steps)
                } else {
                    None
                }
            }
            other => panic!("Unexpected response to Settle: {other:?}"),
        }
    }

    /// Poll `GetTree` until a tree is available (the viewer needs a frame or two before
    /// `AccessKit` produces its first tree). Returns the tree and the viewer's `pixels_per_point`.
    pub(super) fn wait_for_first_tree(&mut self) -> (TreeUpdate, f32) {
        let deadline = Instant::now() + CONNECT_TIMEOUT;
        loop {
            if let Some(update) = self.get_tree() {
                return update;
            }
            assert!(
                Instant::now() < deadline,
                "The viewer never produced an `AccessKit` tree within {CONNECT_TIMEOUT:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Returns the `AccessKit` tree and the viewer's `pixels_per_point` (needed to convert bounds
    /// from physical pixels to logical points for `egui` input events).
    pub(super) fn get_tree(&mut self) -> Option<(TreeUpdate, f32)> {
        match self.request(Request::GetTree) {
            Response::Tree {
                accesskit,
                pixels_per_point,
                ..
            } => accesskit.map(|update| (update, pixels_per_point)),
            other => panic!("Unexpected response to GetTree: {other:?}"),
        }
    }

    pub(super) fn apply_events(&mut self, events: Vec<egui::Event>) {
        match self.request(Request::ApplyEvents { events }) {
            Response::Done => {}
            other => panic!("Unexpected response to ApplyEvents: {other:?}"),
        }
    }

    pub(super) fn resize(&mut self, width: u32, height: u32) {
        // On wasm egui_inspection can't set the size from within the app
        #[cfg(feature = "browser")]
        if let Self::Browser(connection) = self {
            connection.set_viewport(width, height);
        }

        match self.request(Request::Resize { width, height }) {
            Response::Done => {}
            other => panic!("Unexpected response to Resize: {other:?}"),
        }
    }

    /// Evaluate `JavaScript` in the browser and return its string result.
    ///
    /// Panics unless this connection targets a browser.
    #[cfg(feature = "browser")]
    pub(super) fn evaluate_js_in_browser(&self, script: &str) -> String {
        match self {
            Self::Browser(connection) => connection.evaluate_js(script),
            _ => panic!("Browser evaluation requires the browser inspection target"),
        }
    }

    /// Capture the current frame as PNG bytes.
    pub(super) fn screenshot(&mut self) -> Vec<u8> {
        // Grab the screenshot via the chrome dev tools, which should be faster
        #[cfg(feature = "browser")]
        if let Self::Browser(connection) = self {
            return connection.screenshot();
        }

        match self.request(Request::GetScreenshot {
            pixels_per_point: None,
        }) {
            Response::Screenshot(png) => png.bytes,
            other => panic!("Unexpected response to GetScreenshot: {other:?}"),
        }
    }
}
