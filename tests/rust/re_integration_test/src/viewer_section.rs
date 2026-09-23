use egui::{PointerButton, Role};
use egui_kittest::kittest::Queryable as _;

use crate::ViewerHarnessExt;

/// A section of the viewer, e.g. the "Blueprint" or "Recording" panel. Every query and action in a section
/// only affects the children of the section.
pub struct ViewerSection<'a, H: ViewerHarnessExt + ?Sized> {
    harness: &'a mut H,
    section_label: Option<&'a str>,
}

impl<'a, H: ViewerHarnessExt + ?Sized> ViewerSection<'a, H> {
    /// A section covering the children of `section_label`, or the whole app when it is `None`.
    pub(crate) fn new(harness: &'a mut H, section_label: Option<&'a str>) -> Self {
        Self {
            harness,
            section_label,
        }
    }

    /// Returns the root node of the section.
    ///
    /// # Panics
    /// Panics if the section label is not found.
    pub fn root(&self) -> egui_kittest::Node<'_> {
        self.harness.section_node(self.section_label)
    }

    /// Returns the only node with the given label.
    ///
    /// # Panics
    /// Panics if there are zero or multiple nodes with the given label.
    pub fn get_label<'n>(&'n self, label: &'n str) -> egui_kittest::Node<'n> {
        self.root().get_by_label(label)
    }

    /// Returns the nth node with the given label.
    ///
    /// # Panics
    /// Panics if there are fewer such nodes than `index`.
    pub fn get_nth_label<'n>(&'n self, label: &'n str, index: usize) -> egui_kittest::Node<'n> {
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

    /// Toggles the collapse arrow of the `index`th list item labelled `label`, e.g. a
    /// visualizer's components in the selection panel.
    ///
    /// The arrow is its own widget ("Expand"/"Collapse") on the item's row.
    ///
    /// # Panics
    /// Panics if there are fewer such items than `index`, or the item has no arrow.
    pub fn toggle_nth_hierarchical_list(&mut self, label: &str, index: usize) {
        let row = self.get_nth_label(label, index).rect();
        let arrow = self
            .root()
            .get_all_by_role(Role::DisclosureTriangle)
            .find(|arrow| row.contains(arrow.rect().center()))
            .unwrap_or_else(|| panic!("'{label}' #{index} has no collapse arrow on its row"));

        // Click where the arrow is drawn (its node is padded), so the pointer ends up where
        // the snapshots expect it.
        let pos = egui::pos2(row.left() + 8.0, row.center().y);
        assert!(arrow.rect().contains(pos));
        for pressed in [true, false] {
            self.harness.queue_event(egui::Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            self.harness.run();
        }
    }

    /// Helper function to get the node with the given label
    fn get_nth_label_inner<'n>(
        &'n self,
        label: &'n str,
        index: Option<usize>,
    ) -> egui_kittest::Node<'n> {
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
        self.harness.queue_event(egui::Event::PointerButton {
            pos: center,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        self.harness.run();
    }

    /// Helper function to end dragging the node with the given label
    fn drop_label_inner(&mut self, label: &str, index: Option<usize>) {
        let node = self.get_nth_label_inner(label, index);

        let pos = node.rect().center();
        self.harness.queue_event(egui::Event::PointerMoved(pos));
        self.harness.queue_event(egui::Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        self.harness.queue_event(egui::Event::PointerGone);
        self.harness.run();
    }
}
