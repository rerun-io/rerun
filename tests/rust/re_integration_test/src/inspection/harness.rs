//! The test-facing harness: spawn a viewer, drive it with `egui` events, query its `AccessKit`
//! tree, and snapshot it.

use std::time::{Duration, Instant};

use egui_kittest::EventQueue;
use egui_kittest::kittest::{Queryable, State};

use crate::ViewerHarnessExt as _;

use super::connection::{Connection, SETTLE_DIAGNOSTIC_MAX_STEPS, SETTLE_MAX_STEPS};
use super::{HarnessConfig, TargetViewer, TestEnv};

/// A test harness that drives a Rerun viewer over `egui_inspection` (in-process or out-of-process).
pub struct InspectionHarness {
    connection: Connection,
    state: State,
    event_queue: EventQueue,
    size: [u32; 2],

    /// The viewer's latest `pixels_per_point`, refreshed with each `AccessKit` tree.
    pixels_per_point: f32,
}

impl<'tree, 'node> Queryable<'tree, 'node, egui_kittest::Node<'tree>> for InspectionHarness
where
    'node: 'tree,
{
    fn queryable_node(&'node self) -> egui_kittest::Node<'tree> {
        egui_kittest::Node::new(self.state.root(), &self.event_queue, self.pixels_per_point)
    }
}

impl InspectionHarness {
    /// Whether the configured inspection target is the browser.
    pub fn is_browser() -> bool {
        TestEnv::get().target == TargetViewer::Browser
    }

    /// Spawn a viewer and connect to it.
    ///
    /// Which viewer is driven is selected by the `RERUN_INTEGRATION_TEST_TARGET` environment
    /// variable — see [`TargetViewer`].
    pub fn spawn(config: HarnessConfig) -> Self {
        let size = config.size();
        let connection = match TestEnv::get().target {
            TargetViewer::InProcess => Connection::spawn_in_process(config),
            TargetViewer::Cli => Connection::spawn_cli(&config),
            TargetViewer::Browser => {
                #[cfg(feature = "browser")]
                {
                    Connection::spawn_browser(&config)
                }
                #[cfg(not(feature = "browser"))]
                {
                    _ = config;
                    panic!(
                        "The `browser` inspection target requires building `re_integration_test` \
                         with `--features browser`."
                    )
                }
            }
        };

        Self::finish_spawn(connection, size)
    }

    /// Shared tail of the spawn paths: wait for the first tree, build the harness, and set the
    /// requested size.
    fn finish_spawn(mut connection: Connection, size: egui::Vec2) -> Self {
        let (update, pixels_per_point) = connection.wait_for_first_tree();
        let state = State::new(update);

        let mut harness = Self {
            connection,
            state,
            event_queue: EventQueue::default(),
            size: [size.x as u32, size.y as u32],
            pixels_per_point,
        };

        harness.resize(size.x as u32, size.y as u32);

        harness
    }

    /// Run the viewer until it stops repainting, and refresh the `AccessKit` tree.
    ///
    /// Panics if we need more than `SETTLE_MAX_STEPS`. We keep stepping up to
    /// `SETTLE_DIAGNOSTIC_MAX_STEPS` first, so the panic can say how many steps it would have
    /// taken: that tells you whether the budget is slightly too tight or the viewer never goes
    /// quiet at all.
    #[track_caller]
    pub fn run(&mut self) {
        match self.run_ok_within(SETTLE_DIAGNOSTIC_MAX_STEPS) {
            Some(steps) if steps <= SETTLE_MAX_STEPS => {}

            Some(steps) => panic!(
                "Harness needed {steps} steps to settle, more than the budget of \
                 {SETTLE_MAX_STEPS}. Use `run_ok` if this is expected."
            ),

            None => panic!(
                "Harness failed to settle within {SETTLE_DIAGNOSTIC_MAX_STEPS} steps. \
                 Use `run_ok` if this is expected."
            ),
        }
    }

    /// Run the viewer until it stops repainting.
    ///
    /// Returns `Some(steps)` when we settle, or `None`, if we failed to settle.
    pub fn run_ok(&mut self) -> Option<u64> {
        self.run_ok_within(SETTLE_MAX_STEPS)
    }

    /// [`Self::run_ok`], but with an explicit step budget.
    fn run_ok_within(&mut self, max_steps: u64) -> Option<u64> {
        let events = std::mem::take(&mut *self.event_queue.lock());
        if !events.is_empty() {
            self.connection.apply_events(events);
        }
        let steps = self.connection.settle(max_steps);
        self.refresh_tree();
        steps
    }

    /// Queue a raw `egui` event, sent to the viewer by the next [`Self::run`].
    ///
    /// Mirrors `egui_kittest::Harness::event`: queue-only, so a caller can build up a batch and
    /// flush it in one frame.
    pub fn queue_event(&self, event: egui::Event) {
        self.event_queue.lock().push(event);
    }

    /// Move the pointer to `pos`. Mirrors `egui_kittest::Harness::hover_at`.
    pub fn hover_at(&self, pos: egui::Pos2) {
        self.queue_event(egui::Event::PointerMoved(pos));
    }

    /// Press the primary button at `pos`, starting a drag.
    /// Mirrors `egui_kittest::Harness::drag_at`.
    pub fn drag_at(&self, pos: egui::Pos2) {
        self.queue_event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
    }

    /// Release the primary button at `pos` and take the cursor off-screen.
    /// Mirrors `egui_kittest::Harness::drop_at`.
    pub fn drop_at(&self, pos: egui::Pos2) {
        self.queue_event(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        self.remove_cursor();
    }

    /// Take the cursor off-screen so it doesn't linger as a hover in the next snapshot.
    /// Mirrors `egui_kittest::Harness::remove_cursor`.
    pub fn remove_cursor(&self) {
        self.queue_event(egui::Event::PointerGone);
    }

    /// Repeatedly [`run_ok`](Self::run_ok) and evaluate `predicate` until it returns `true` or
    /// `timeout` elapses.
    ///
    /// Failing to settle is not an error here: we are waiting for something to happen, so the
    /// viewer is expected to still be repainting. `timeout` is the real deadline.
    pub fn step_until_with_custom_timeout(
        &mut self,
        description: &str,
        mut predicate: impl FnMut(&Self) -> bool,
        poll_interval: Duration,
        timeout: Duration,
    ) {
        let start = Instant::now();
        self.run_ok();
        while !predicate(self) {
            assert!(
                start.elapsed() <= timeout,
                "Timed out waiting for {description:?} after {timeout:?}.\n\
                 Available nodes: {:#?}",
                self.root_node()
            );
            std::thread::sleep(poll_interval);
            self.run_ok();
        }
    }

    /// Resize the viewer's viewport to the given logical-point dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.connection.resize(width, height);
        self.size = [width, height];
        // The resize takes effect on the viewer's next frame, and relayout may take a few more.
        self.connection.settle(SETTLE_MAX_STEPS);
        self.refresh_tree();
    }

    /// Evaluate an async `JavaScript` expression in the browser target.
    #[cfg(feature = "browser")]
    pub fn evaluate_js_in_browser(&self, script: &str) -> String {
        self.connection.evaluate_js_in_browser(script)
    }

    /// Capture the current frame as an image.
    pub fn screenshot(&mut self) -> image::RgbaImage {
        let png = self.connection.screenshot();
        image::load_from_memory(&png)
            .expect("Failed to decode screenshot PNG")
            .to_rgba8()
    }

    /// Capture a screenshot and compare it against the snapshot named `name`.
    pub fn try_snapshot(&mut self, name: &str) -> egui_kittest::SnapshotResult {
        let image = self.screenshot();
        egui_kittest::try_image_snapshot_options(&image, name, &Self::snapshot_options())
    }

    /// The snapshot tolerances for the viewer we are driving.
    ///
    /// The in-process viewer renders through the same `wgpu` setup as our other snapshot tests, so
    /// it keeps the strict defaults and a regression there still fails the build.
    ///
    /// The out-of-process targets render the same UI with a different rasterizer — the browser goes
    /// through Chrome's `SwiftShader` rather than `lavapipe` — which shifts a few hundred antialiased
    /// pixels. Raising the per-pixel threshold does not help: at 0.6 the browser still differs in 255
    /// pixels and at 10.0 in 182, so these are genuinely different colors rather than near misses.
    /// The count is the knob instead. The worst observed on CI is 240 (`dataset_ui_empty_form`).
    fn snapshot_options() -> egui_kittest::SnapshotOptions {
        let options = re_ui::testing::default_snapshot_options_for_ui();
        match TestEnv::get().target {
            TargetViewer::InProcess => options,
            TargetViewer::Cli | TargetViewer::Browser => options.max_failed_pixels(500),
        }
    }

    fn refresh_tree(&mut self) {
        if let Some((update, pixels_per_point)) = self.connection.get_tree() {
            self.state.update(update);
            self.pixels_per_point = pixels_per_point;
        }
    }
}
