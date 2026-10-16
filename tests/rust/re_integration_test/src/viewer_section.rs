use egui::PointerButton;
use egui_kittest::kittest::Queryable as _;

use crate::ViewerHarnessExt;

/// A section of the viewer, e.g. the "Blueprint" or "Recording" panel. Every query and action in a section
/// only affects the children of the section.
pub struct ViewerSection<'a, H: ViewerHarnessExt + ?Sized> {
    harness: &'a mut H,
    section_label: Option<&'a str>,
}

impl<'a, H: ViewerHarnessExt + ?Sized> ViewerSection<'a, H> {
    fn list_item_interaction_position(&self, row_rect: egui::Rect) -> egui::Pos2 {
        let center = row_rect.center();
        let preferred = egui::pos2((row_rect.left() + 24.0).min(center.x), center.y);
        let Some(collapse_rect) = std::iter::chain(
            self.root().query_all_by_label("Collapse"),
            self.root().query_all_by_label("Expand"),
        )
        .find(|node| node.rect().y_range().contains(center.y))
        .map(|node| node.rect()) else {
            return preferred;
        };

        if !collapse_rect.contains(preferred) {
            return preferred;
        }

        [
            collapse_rect.left_center() - egui::vec2(1.0, 0.0),
            collapse_rect.right_center() + egui::vec2(1.0, 0.0),
        ]
        .into_iter()
        .filter(|pos| row_rect.contains(*pos))
        .min_by(|a, b| a.distance_sq(center).total_cmp(&b.distance_sq(center)))
        .unwrap_or(center)
    }

    fn click_at(&mut self, pos: egui::Pos2, button: PointerButton, modifiers: egui::Modifiers) {
        self.harness.queue_event(egui::Event::PointerMoved(pos));
        self.harness
            .queue_event(egui::Event::ModifiersChanged(modifiers));
        for pressed in [true, false] {
            self.harness.queue_event(egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers,
            });
        }
        self.harness
            .queue_event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        self.harness.run();
    }

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

    /// Clicks a list item while avoiding its collapse button.
    ///
    /// # Panics
    /// Panics if there are zero or multiple nodes with the given label.
    pub fn click_list_item(&mut self, label: &str) {
        let pos = self.list_item_interaction_position(self.root().get_by_label(label).rect());
        self.click_at(pos, PointerButton::Primary, egui::Modifiers::NONE);
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

    /// Clicks the nth list item while avoiding its collapse button.
    ///
    /// # Panics
    /// Panics if there are fewer such nodes than `index`.
    pub fn click_nth_list_item(&mut self, label: &str, index: usize) {
        let pos = self.list_item_interaction_position(self.get_nth_label(label, index).rect());
        self.click_at(pos, PointerButton::Primary, egui::Modifiers::NONE);
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

    /// Clicks a list item with modifiers while avoiding its collapse button.
    ///
    /// # Panics
    /// Panics if there are zero or multiple nodes with the given label.
    pub fn click_list_item_modifiers(&mut self, label: &str, modifiers: egui::Modifiers) {
        let pos = self.list_item_interaction_position(self.root().get_by_label(label).rect());
        self.click_at(pos, PointerButton::Primary, modifiers);
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
        let row_rect = self.get_nth_label(label, index).rect();
        let arrow_rect = std::iter::chain(
            self.root().query_all_by_label("Collapse"),
            self.root().query_all_by_label("Expand"),
        )
        .find(|node| node.rect().y_range().contains(row_rect.center().y))
        .unwrap_or_else(|| panic!("'{label}' #{index} has no collapse arrow on its row"))
        .rect();

        // Press and release must be flushed by the same `run`. Settling in between lets a busy
        // viewer hold the button for several frames, and once that exceeds egui's
        // `max_click_duration` the release is no longer a click.
        self.click_at(
            arrow_rect.center(),
            PointerButton::Primary,
            egui::Modifiers::NONE,
        );
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
