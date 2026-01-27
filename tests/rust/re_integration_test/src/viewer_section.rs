use egui::PointerButton;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;

use crate::HarnessExt as _;

/// A section of the viewer, e.g. the "Blueprint" or "Recording" panel. Every query and action in a section
/// only affects the children of the section.
pub struct ViewerSection<'a, 'h> {
    pub harness: &'a mut egui_kittest::Harness<'h, re_viewer::App>,
    pub section_label: Option<&'a str>,
}

impl<'a, 'h: 'a> ViewerSection<'a, 'h> {
    /// Returns the root node of the section.
    ///
    /// # Panics
    /// Panics if the section label is not found.
    pub fn root<'n>(&'n self) -> egui_kittest::Node<'n>
    where
        'a: 'n,
    {
        let Some(section_label) = self.section_label else {
            return self.harness.root();
        };
        self.harness
            .get_by_role_and_label(Role::Pane, section_label)
    }

    /// Returns the only node with the given label.
    ///
    /// # Panics
    /// Panics if there are zero or multiple nodes with the given label.
    pub fn get_label<'n>(&'n self, label: &'n str) -> egui_kittest::Node<'n>
    where
        'a: 'n,
    {
        self.root().get_by_label(label)
    }

    /// Returns the nth node with the given label.
    ///
    /// # Panics
    /// Panics if there are fewer such nodes than `index`.
    pub fn get_nth_label<'n>(&'n self, label: &'n str, index: usize) -> egui_kittest::Node<'n>
    where
        'a: 'n,
    {
        let mut nodes = self.root().get_all_by_label(label).collect::<Vec<_>>();
        assert!(
            index < nodes.len(),
            "Failed to find label '{label}' #{index}, there are only {} nodes:\n{nodes:#?}",
            nodes.len()
        );
        nodes.swap_remove(index)
    }

    /// Clicks the only node with the given label.
    ///
    /// # Panics
    /// Panics if there are zero or multiple nodes with the given label.
    pub fn click_label(&mut self, label: &str) {
        self.root().get_by_label(label).click();
        self.harness.run();
    }

    /// Right-clicks the only node with the given label.
    ///
    /// # Panics
    /// Panics if there are zero or multiple nodes with the given label.
    pub fn right_click_label(&mut self, label: &str) {
        self.root().get_by_label(label).click_secondary();
        self.harness.run();
    }

    /// Clicks the nth node with the given label.
    ///
    /// # Panics
    /// Panics if there are fewer such nodes than `index`.
    pub fn click_nth_label(&mut self, label: &str, index: usize) {
        self.get_nth_label(label, index).click();
        self.harness.run();
    }

    /// Right-clicks the nth node with the given label.
    ///
    /// # Panics
    /// Panics if there are fewer such nodes than `index`.
    pub fn right_click_nth_label(&mut self, label: &str, index: usize) {
        self.get_nth_label(label, index).click_secondary();
        self.harness.run();
    }

    /// Clicks the only node with the given label using modifiers.
    ///
    /// # Panics
    /// Panics if there are zero or multiple nodes with the given label.
    pub fn click_label_modifiers(&mut self, label: &str, modifiers: egui::Modifiers) {
        self.root().get_by_label(label).click_modifiers(modifiers);
        self.harness.run();
    }

    /// Clicks the only node with the label that contains the given text.
    ///
    /// # Panics
    /// Panics if there are zero or multiple nodes with the given label.
    pub fn click_label_contains(&mut self, label: &str) {
        self.root().get_by_label_contains(label).click();
        self.harness.run();
    }

    /// Starts dragging the nth node with the given label.
    ///
    /// # Panics
    /// Panics if there are fewer such nodes than `index`.
    pub fn drag_nth_label(&mut self, label: &str, index: usize) {
        self.drag_label_inner(label, Some(index));
    }

    /// Starts dragging the only node with the given label.
    ///
    /// # Panics
    /// Panics if there are zero or multiple nodes with the given label.
    pub fn drag_label(&mut self, label: &str) {
        self.drag_label_inner(label, None);
    }

    /// Ends dragging over the only node with the given label.
    ///
    /// # Panics
    /// Panics if there are zero or multiple nodes with the given label.
    pub fn drop_label(&mut self, label: &str) {
        self.drop_label_inner(label, None);
    }

    /// Ends dragging over the nth node with the given label.
    ///
    /// # Panics
    /// Panics if there are fewer such nodes than `index`.
    pub fn drop_nth_label(&mut self, label: &str, index: usize) {
        self.drop_label_inner(label, Some(index));
    }

    /// Hover over the only node with the given label.
    ///
    /// # Panics
    /// Panics if there are zero or multiple nodes with the given label.
    pub fn hover_label(&mut self, label: &str) {
        self.get_label(label).hover();
        self.harness.run();
    }

    /// Hover over the nth node with the given label.
    ///
    /// # Panics
    /// Panics if there are fewer such nodes than `index`.
    pub fn hover_nth_label(&mut self, label: &str, index: usize) {
        self.get_nth_label(label, index).hover();
        self.harness.run();
    }

    /// Hover over the only node with the label that contains the given text.
    ///
    /// # Panics
    /// Panics if there are zero or multiple nodes with the given label.
    pub fn hover_label_contains(&mut self, label: &str) {
        self.root().get_by_label_contains(label).hover();
        self.harness.run();
    }

    /// Toggles the collapse triangle of a hierarchical list item. Eg. visualizer components in the selection panel.
    pub fn toggle_nth_hierarchical_list(&mut self, label: &str, index: usize) {
        let node = self.get_nth_label(label, index);
        let rect = node.rect();

        // Click at the left edge of the rect + 8 pixels (to hit the center of the ~16px triangle)
        let triangle_x = rect.left() + 8.0;
        let triangle_y = rect.center().y;
        let triangle_pos = egui::pos2(triangle_x, triangle_y);
        self.harness.click_at(triangle_pos);
    }

    /// Helper function to get the node with the given label
    fn get_nth_label_inner<'n>(
        &'n self,
        label: &'n str,
        index: Option<usize>,
    ) -> egui_kittest::Node<'n>
    where
        'a: 'n,
    {
        if let Some(index) = index {
            self.get_nth_label(label, index)
        } else {
            self.get_label(label)
        }
    }

    /// Helper function to start dragging the node with the given label
    fn drag_label_inner(&mut self, label: &str, index: Option<usize>) {
        let node = self.get_nth_label_inner(label, index);

        let center = node.rect().center();
        self.harness.event(egui::Event::PointerButton {
            pos: center,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });

        // Step until the time has passed `max_click_duration` so this gets
        // registered as a drag.
        let wait_time = self
            .harness
            .ctx
            .options(|o| o.input_options.max_click_duration);
        let end_time = self.harness.ctx.input(|i| i.time + wait_time);
        while self.harness.ctx.input(|i| i.time) < end_time {
            self.harness.step();
        }
    }

    /// Helper function to end dragging the node with the given label
    pub fn drop_label_inner(&mut self, label: &str, index: Option<usize>) {
        let node = self.get_nth_label_inner(label, index);
        let event = egui::Event::PointerButton {
            pos: node.rect().center(),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        };
        self.harness.event(event);
        self.harness.remove_cursor();
        self.harness.run();
    }
}
