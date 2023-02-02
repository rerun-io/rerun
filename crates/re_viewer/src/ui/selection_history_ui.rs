use egui::RichText;
use re_ui::Command;

use super::{HistoricalSelection, SelectionHistory};
use crate::{misc::MultiSelection, ui::Blueprint, Selection};

// ---

impl SelectionHistory {
    pub(crate) fn selection_ui(
        &mut self,
        ui: &mut egui::Ui,
        blueprint: &Blueprint,
    ) -> Option<MultiSelection> {
        ui
            // so the strip doesn't try and occupy the entire vertical space
            .horizontal(|ui| self.control_bar_ui(ui, blueprint))
            .inner
            .map(|sel| sel.selection)
    }

    fn control_bar_ui(
        &mut self,
        ui: &mut egui::Ui,
        blueprint: &Blueprint,
    ) -> Option<HistoricalSelection> {
        ui.horizontal(|ui|{
            ui.strong("Selection").on_hover_text("The Selection View contains information and options about the currently selected object(s).");

            // TODO(emilk): an egui helper for right-to-left
            ui.allocate_ui_with_layout(
                egui::vec2(
                    ui.available_size_before_wrap().x,
                    ui.spacing().interact_size.y,
                ),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    let next = self.next_button_ui(ui, blueprint);
                    let prev = self.prev_button_ui(ui, blueprint);
                    prev.or(next)
                }).inner
        }).inner
    }

    // TODO(cmc): note that for now, we only check prev/next shortcuts in the UI code that
    // shows the associated buttons... this means shortcuts only work when the selection panel
    // is open!
    // We might want to change this at some point, though the way things are currently designed,
    // there isn't much point in selecting stuff while the selection panel is hidden anyway.

    pub fn select_previous(&mut self) -> Option<HistoricalSelection> {
        if let Some(previous) = self.previous() {
            if previous.index != self.current {
                self.current = previous.index;
                return self.current();
            }
        }
        None
    }

    pub fn select_next(&mut self) -> Option<HistoricalSelection> {
        if let Some(next) = self.next() {
            if next.index != self.current {
                self.current = next.index;
                return self.current();
            }
        }
        None
    }

    fn prev_button_ui(
        &mut self,
        ui: &mut egui::Ui,
        blueprint: &Blueprint,
    ) -> Option<HistoricalSelection> {
        // undo selection
        let button = egui::Button::new("⬅");
        if let Some(previous) = self.previous() {
            let response = ui.add(button).on_hover_text(format!(
                "Go to previous selection{}:\n\
                {}\n\
                \n\
                Right-click for more.",
                Command::SelectionPrevious.format_shortcut_tooltip_suffix(ui.ctx()),
                multi_selection_to_string(blueprint, &previous.selection),
            ));

            let response = response.context_menu(|ui| {
                // undo: newest on top, oldest on bottom
                for i in (0..self.current).rev() {
                    self.history_item_ui(blueprint, ui, i);
                }
            });

            // TODO(cmc): using the keyboard shortcut should highlight the associated
            // button or something (but then again it, it'd make more sense to do that
            // at the egui level rather than specifically here).
            if response.clicked() {
                return self.select_previous();
            }
        } else {
            ui.add_enabled(false, button)
                .on_disabled_hover_text("No past selections found");
        }

        None
    }

    fn next_button_ui(
        &mut self,
        ui: &mut egui::Ui,
        blueprint: &Blueprint,
    ) -> Option<HistoricalSelection> {
        // redo selection
        let button = egui::Button::new("➡");
        if let Some(next) = self.next() {
            let response = ui.add(button).on_hover_text(format!(
                "Go to next selection{}:\n\
                {}\n\
                \n\
                Right-click for more.",
                Command::SelectionNext.format_shortcut_tooltip_suffix(ui.ctx()),
                multi_selection_to_string(blueprint, &next.selection),
            ));

            let response = response.context_menu(|ui| {
                // redo: oldest on top, most recent on bottom
                for i in (self.current + 1)..self.stack.len() {
                    self.history_item_ui(blueprint, ui, i);
                }
            });

            // TODO(cmc): using the keyboard shortcut should highlight the associated
            // button or something (but then again it, it'd make more sense to do that
            // at the egui level rather than specifically here).
            if response.clicked() {
                return self.select_next();
            }
        } else {
            ui.add_enabled(false, button)
                .on_disabled_hover_text("No future selections found");
        }

        None
    }

    fn history_item_ui(&mut self, blueprint: &Blueprint, ui: &mut egui::Ui, index: usize) {
        if let Some(sel) = self.stack.get(index) {
            ui.horizontal(|ui| {
                {
                    // borrow checker workaround
                    let sel = multi_selection_to_string(blueprint, sel);
                    if ui.selectable_value(&mut self.current, index, sel).clicked() {
                        ui.close_menu();
                    }
                }
                if sel.len() == 1 {
                    selection_kind_ui(ui, sel.iter().next().unwrap());
                }
            });
        }
    }
}

// Different kinds of selections can share the same path in practice! We need to
// differentiate those in the UI to avoid confusion.
fn selection_kind_ui(ui: &mut egui::Ui, sel: &Selection) {
    ui.weak(RichText::new(format!("({})", sel.kind())));
}

fn multi_selection_to_string(blueprint: &Blueprint, sel: &MultiSelection) -> String {
    assert!(!sel.is_empty()); // history never contains empty selections.
    if sel.len() == 1 {
        single_selection_to_string(blueprint, sel.iter().next().unwrap())
    } else if let Some(kind) = sel.are_all_same_kind() {
        format!("{}x {}s", sel.len(), kind)
    } else {
        "<multiple selections>".to_owned()
    }
}

fn single_selection_to_string(blueprint: &Blueprint, sel: &Selection) -> String {
    match sel {
        Selection::SpaceView(sid) => {
            if let Some(space_view) = blueprint.viewport.space_view(sid) {
                space_view.name.clone()
            } else {
                "<removed space view>".to_owned()
            }
        }
        Selection::InstancePath(_, entity_path) => entity_path.to_string(),
        Selection::DataBlueprintGroup(sid, handle) => {
            if let Some(space_view) = blueprint.viewport.space_view(sid) {
                if let Some(group) = space_view.data_blueprint.group(*handle) {
                    group.display_name.clone()
                } else {
                    format!("<removed group in {}>", space_view.name)
                }
            } else {
                "<group in removed space view>".to_owned()
            }
        }
        Selection::MsgId(msg_id) => msg_id.short_string(),
        Selection::ComponentPath(path) => {
            format!("{} {}", path.entity_path, path.component_name.short_name(),)
        }
    }
}
