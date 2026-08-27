use std::time::Duration;

use egui::accesskit::Toggled;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use crate::InspectionHarness;

/// Convenience helpers shared by both test harnesses.
pub trait ViewerHarnessExt {
    /// Flush queued input and let the viewer settle.
    fn run(&mut self);

    /// Queue a raw `egui` event, sent to the viewer by the next [`Self::run`].
    ///
    /// The node helpers cover most interaction; this is for input that isn't tied to a node, such
    /// as moving the pointer to a bare position mid-drag.
    fn queue_event(&self, event: egui::Event);

    /// The root query node of the viewer section with the given (normally-hidden) pane label, or
    /// the whole-app root when `None`.
    fn section_node<'s>(&'s self, section_label: Option<&'s str>) -> egui_kittest::Node<'s>;

    /// The whole-app root query node.
    fn root_node(&self) -> egui_kittest::Node<'_> {
        self.section_node(None)
    }

    /// Scope subsequent queries and interactions to the viewer section with the given
    /// (normally-hidden) pane label. See [`crate::ViewerSection`].
    fn section<'a>(&'a mut self, section_label: &'a str) -> crate::ViewerSection<'a, Self>
    where
        Self: Sized,
    {
        crate::ViewerSection::new(self, Some(section_label))
    }

    /// The section spanning the whole app.
    fn root_section(&mut self) -> crate::ViewerSection<'_, Self>
    where
        Self: Sized,
    {
        crate::ViewerSection::new(self, None)
    }

    /// The blueprint tree section.
    fn blueprint_tree(&mut self) -> crate::ViewerSection<'_, Self>
    where
        Self: Sized,
    {
        self.section("_blueprint_tree")
    }

    /// The streams tree section.
    fn streams_tree(&mut self) -> crate::ViewerSection<'_, Self>
    where
        Self: Sized,
    {
        self.section("_streams_tree")
    }

    /// The recording ("Sources") panel section.
    fn recording_panel(&mut self) -> crate::ViewerSection<'_, Self>
    where
        Self: Sized,
    {
        self.section("_recording_panel")
    }

    /// The selection panel section.
    fn selection_panel(&mut self) -> crate::ViewerSection<'_, Self>
    where
        Self: Sized,
    {
        self.section("_selection_panel")
    }

    /// Is the viewer showing any loading indicator?
    ///
    /// Checks if there are any [`egui::accesskit::Role::ProgressIndicator`] shown.
    fn is_loading(&self) -> bool {
        self.root_node()
            .query_all_by_role(egui::accesskit::Role::ProgressIndicator)
            .next()
            .is_some()
    }

    /// Repeatedly [`run`](Self::run) and evaluate `predicate` until it returns `true` or the
    /// default timeout elapses.
    #[track_caller]
    fn step_until(&mut self, description: &'static str, predicate: impl FnMut(&Self) -> bool) {
        self.step_until_with_custom_timeout(
            description,
            predicate,
            re_viewer::viewer_test_utils::DEFAULT_POLL_INTERVAL,
            re_viewer::viewer_test_utils::DEFAULT_WAIT_TIMEOUT,
        );
    }

    /// Repeatedly [`run`](Self::run) and evaluate `predicate` until it returns `true` or
    /// `timeout` elapses.
    ///
    /// Failing to settle is not an error here: we are waiting for something to happen, so the
    /// viewer is expected to still be repainting. `timeout` is the real deadline.
    #[track_caller]
    fn step_until_with_custom_timeout(
        &mut self,
        description: &'static str,
        predicate: impl FnMut(&Self) -> bool,
        poll_interval: Duration,
        timeout: Duration,
    );

    /// Wait until the viewer is no longer showing any loading indicator.
    #[track_caller]
    fn step_until_no_loading_indicator(&mut self) {
        self.step_until("Wait until there's no more loading indicator", |harness| {
            !harness.is_loading()
        });
    }

    /// Click the only node with `label`, then settle.
    ///
    /// # Panics
    /// Panics if there are zero or multiple matching nodes.
    fn click_label(&mut self, label: &str) {
        self.root_node().get_by_label(label).click();
        self.run();
    }

    /// Right-click the only node with `label`, then settle.
    ///
    /// # Panics
    /// Panics if there are zero or multiple matching nodes.
    fn right_click_label(&mut self, label: &str) {
        self.root_node().get_by_label(label).click_secondary();
        self.run();
    }

    /// Click the only node whose label contains `label`, then settle.
    ///
    /// # Panics
    /// Panics if there are zero or multiple matching nodes.
    fn click_label_contains(&mut self, label: &str) {
        self.root_node().get_by_label_contains(label).click();
        self.run();
    }

    /// Hover the only node whose label contains `label`, then settle.
    ///
    /// # Panics
    /// Panics if there are zero or multiple matching nodes.
    fn hover_label_contains(&mut self, label: &str) {
        self.root_node().get_by_label_contains(label).hover();
        self.run();
    }

    /// Open or close a viewer panel identified by its toggle-button label. No-op if already in the
    /// requested state.
    fn set_panel_opened(&mut self, panel_label: &str, opened: bool) {
        let is_open = self
            .root_node()
            .get_by_label(panel_label)
            .accesskit_node()
            .data()
            .toggled()
            == Some(Toggled::True);
        if is_open != opened {
            self.root_node().get_by_label(panel_label).click();
        }
        self.run();
    }

    /// Open or close the blueprint (left) panel.
    fn set_blueprint_panel_opened(&mut self, opened: bool) {
        self.set_panel_opened("Blueprint panel toggle", opened);
    }

    /// Open or close the selection (right) panel.
    fn set_selection_panel_opened(&mut self, opened: bool) {
        self.set_panel_opened("Selection panel toggle", opened);
    }

    /// Open or close the time (bottom) panel.
    fn set_time_panel_opened(&mut self, opened: bool) {
        self.set_panel_opened("Time panel toggle", opened);
    }
}

impl ViewerHarnessExt for egui_kittest::Harness<'_, re_viewer::App> {
    fn run(&mut self) {
        egui_kittest::Harness::run_ok(self);
    }

    fn queue_event(&self, event: egui::Event) {
        egui_kittest::Harness::event(self, event);
    }

    fn section_node<'s>(&'s self, section_label: Option<&'s str>) -> egui_kittest::Node<'s> {
        match section_label {
            None => self.root(),
            Some(label) => self.get_by_role_and_label(egui::accesskit::Role::Pane, label),
        }
    }

    #[track_caller]
    fn step_until_with_custom_timeout(
        &mut self,
        description: &'static str,
        mut predicate: impl FnMut(&Self) -> bool,
        poll_interval: Duration,
        timeout: Duration,
    ) {
        re_viewer::viewer_test_utils::step_until_with_custom_timeout(
            description,
            self,
            |harness| predicate(&*harness),
            poll_interval,
            timeout,
        );
    }

    // The in-process harness removes the cursor after toggling so it doesn't linger over the button
    // in the next snapshot, and settles fully via `run_ok`.
    fn set_panel_opened(&mut self, panel_label: &str, opened: bool) {
        let is_open = Some(Toggled::True)
            == self
                .get_by_label(panel_label)
                .accesskit_node()
                .data()
                .toggled();
        if is_open != opened {
            self.get_by_label(panel_label).click();
        }
        self.remove_cursor();
        self.run_ok();
    }
}

impl ViewerHarnessExt for InspectionHarness {
    // The explicit path calls `InspectionHarness`'s inherent `run` (not this trait method).
    #[expect(clippy::use_self)]
    fn run(&mut self) {
        InspectionHarness::run(self);
    }

    // As with `run` above, the explicit path calls the inherent method, not this trait method.
    #[expect(clippy::use_self)]
    fn queue_event(&self, event: egui::Event) {
        InspectionHarness::queue_event(self, event);
    }

    fn section_node<'s>(&'s self, section_label: Option<&'s str>) -> egui_kittest::Node<'s> {
        match section_label {
            None => self.queryable_node(),
            Some(label) => self.get_by_label(label),
        }
    }

    #[track_caller]
    fn step_until_with_custom_timeout(
        &mut self,
        description: &'static str,
        predicate: impl FnMut(&Self) -> bool,
        poll_interval: Duration,
        timeout: Duration,
    ) {
        Self::step_until_with_custom_timeout(self, description, predicate, poll_interval, timeout);
    }
}
