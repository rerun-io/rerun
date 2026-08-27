//! Per-test configuration for a spawned viewer.

/// Configuration for [`InspectionHarness::spawn`](super::InspectionHarness::spawn).
#[derive(Clone, Debug, Default)]
pub struct HarnessConfig {
    /// Logical size of the viewer, in points.
    ///
    /// Defaults to 1024×768, matching the in-process `viewer_harness`.
    pub size: Option<egui::Vec2>,

    /// A URL to open on startup (e.g. `rerun+http://localhost:{port}/entry/{id}`).
    ///
    /// Opened on the in-process viewer via `HarnessOptions::startup_url`, passed as a positional
    /// argument to the `cli` viewer, and via the `?url=` query parameter to the browser viewer.
    pub startup_url: Option<String>,
}

impl HarnessConfig {
    pub(super) fn size(&self) -> egui::Vec2 {
        self.size.unwrap_or(egui::vec2(1024.0, 768.0))
    }
}
