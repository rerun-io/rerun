use std::collections::HashSet;

use re_ui::{UiExt as _, list_item};

#[derive(Hash, Clone, Copy, PartialEq, Eq)]
struct ItemId(u32);

pub struct ExampleDragAndDrop {
    items: Vec<ItemId>,

    /// currently selected items
    selected_items: HashSet<ItemId>,
}

impl Default for ExampleDragAndDrop {
    fn default() -> Self {
        Self {
            items: (0..10).map(ItemId).collect(),
            selected_items: HashSet::new(),
        }
    }
}

impl ExampleDragAndDrop {
    /// Draw the drag-and-drop demo.
    ///
    /// Note: this function uses `ListItem` and must be wrapped in a `ListItemContent`.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let mut swap: Option<(usize, usize)> = None;

        for (i, item_id) in self.items.iter().enumerate() {
            //
            // Draw the item
            //

            let label = format!("Item {}", item_id.0);
            let response = list_item::ListItem::new()
                .selected(self.selected_items.contains(item_id))
                .draggable(true)
                .show_flat(ui, list_item::LabelContent::new(&label));

            //
            // Handle item selection
            //

            // Basic click and cmd/ctr-click
            if response.clicked() {
                if ui.input(|i| i.modifiers.command) {
                    if self.selected_items.contains(item_id) {
                        self.selected_items.remove(item_id);
                    } else {
                        self.selected_items.insert(*item_id);
                    }
                } else {
                    self.selected_items.clear();
                    self.selected_items.insert(*item_id);
                }
            }

            // Drag-and-drop of multiple items not (yet?) supported, so dragging resets selection to single item.
            if response.drag_started() {
                self.selected_items.clear();
                self.selected_items.insert(*item_id);

                response.dnd_set_drag_payload(i);
            }

            //
            // Detect drag situation and run the swap if it ends.
            //

            let source_item_position_index = egui::DragAndDrop::payload(ui.ctx()).map(|i| *i);

            if let Some(source_item_position_index) = source_item_position_index {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);

                let (top, bottom) = response.rect.split_top_bottom_at_fraction(0.5);

                let (insert_y, target) = if ui.rect_contains_pointer(top) {
                    (Some(top.top()), Some(i))
                } else if ui.rect_contains_pointer(bottom) {
                    (Some(bottom.bottom()), Some(i + 1))
                } else {
                    (None, None)
                };

                if let (Some(insert_y), Some(target)) = (insert_y, target) {
                    ui.painter().hline(
                        ui.cursor().x_range(),
                        insert_y,
                        (2.0, ui.tokens().strong_fg_color),
                    );

                    // note: can't use `response.drag_released()` because we not the item which
                    // started the drag
                    if ui.input(|i| i.pointer.any_released()) {
                        swap = Some((source_item_position_index, target));

                        egui::DragAndDrop::clear_payload(ui.ctx());
                    }
                }
            }
        }

        //
        // Handle the swap command (if any)
        //

        if let Some((source, target)) = swap
            && source != target
        {
            let item = self.items.remove(source);

            if source < target {
                self.items.insert(target - 1, item);
            } else {
                self.items.insert(target, item);
            }
        }
    }
}
