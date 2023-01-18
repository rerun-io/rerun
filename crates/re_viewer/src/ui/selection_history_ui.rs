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
        use egui_extras::{Size, StripBuilder};

        const BUTTON_SIZE: f32 = 20.0;
        const MIN_COMBOBOX_SIZE: f32 = 100.0;

        let font_id = egui::TextStyle::Body.resolve(ui.style());

        let mut res = None;
        StripBuilder::new(ui)
            .cell_layout(egui::Layout::centered_and_justified(
                egui::Direction::TopDown, // whatever
            ))
            .size(Size::exact(BUTTON_SIZE)) // prev
            .size(Size::remainder().at_least(MIN_COMBOBOX_SIZE)) // browser
            .size(Size::exact(BUTTON_SIZE)) // next
            .horizontal(|mut strip| {
                // prev
                let mut prev = None;
                strip.cell(|ui| {
                    prev = self.prev_button_ui(ui, blueprint);
                });

                // browser
                let mut picked = None;
                strip.cell(|ui| {
                    let clipped_width = ui.available_width() - 32.0; // leave some space for the drop-down icon!
                    picked = egui::ComboBox::from_id_source("history_browser")
                        .width(ui.available_width())
                        .wrap(false)
                        // TODO(cmc): ideally I would want this to show full selection string
                        // on hover (needs egui patch).
                        .selected_text(self.current().map_or_else(String::new, |sel| {
                            selection_to_clipped_string(
                                ui,
                                blueprint,
                                &sel.selection,
                                &font_id,
                                clipped_width,
                            )
                        }))
                        .show_ui(ui, |ui| {
                            for (i, sel) in self.stack.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    selection_index_ui(ui, i);
                                    {
                                        // borrow checker workaround
                                        let sel = multi_selection_to_string(blueprint, sel);
                                        ui.selectable_value(&mut self.current, i, sel);
                                    }
                                    if sel.selected().len() == 1 {
                                        selection_kind_ui(ui, sel.selected().first().unwrap());
                                    }
                                });
                            }
                        })
                        .inner
                        .and_then(|_| self.current());
                });

                // next
                let mut next = None;
                strip.cell(|ui| {
                    next = self.next_button_ui(ui, blueprint);
                });

                res = prev.or(picked).or(next);
            });

        res
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
        const PREV_BUTTON: &str = "⏴";
        if let Some(previous) = self.previous() {
            let button_clicked = ui
                .small_button(PREV_BUTTON)
                .on_hover_text(format!(
                    "Go to previous selection{}:\n[{}] {}",
                    Command::SelectionPrevious.format_shortcut_tooltip_suffix(ui.ctx()),
                    previous.index,
                    multi_selection_to_string(blueprint, &previous.selection),
                ))
                .clicked();
            // TODO(cmc): feels like using the shortcut should highlight the associated
            // button or something (but then again it, it'd make more sense to do that
            // at the egui level rather than specifically here).
            if button_clicked {
                return self.select_previous();
            }
        } else {
            // Creating a superfluous horizontal UI so that we can still have hover text.
            ui.horizontal(|ui| ui.add_enabled(false, egui::Button::new(PREV_BUTTON)))
                .response
                .on_hover_text("No past selections found");
        }

        None
    }

    fn next_button_ui(
        &mut self,
        ui: &mut egui::Ui,
        blueprint: &Blueprint,
    ) -> Option<HistoricalSelection> {
        const NEXT_BUTTON: &str = "⏵";
        if let Some(next) = self.next() {
            let button_clicked = ui
                .small_button(NEXT_BUTTON)
                .on_hover_text(format!(
                    "Go to next selection{}:\n[{}] {}",
                    Command::SelectionNext.format_shortcut_tooltip_suffix(ui.ctx()),
                    next.index,
                    multi_selection_to_string(blueprint, &next.selection),
                ))
                .clicked();
            // TODO(cmc): feels like using the shortcut should highlight the associated
            // button or something (but then again it, it'd make more sense to do that
            // at the egui level rather than specifically here).
            if button_clicked {
                return self.select_next();
            }
        } else {
            // Creating a superfluous horizontal UI so that we can still have hover text.
            ui.horizontal(|ui| ui.add_enabled(false, egui::Button::new(NEXT_BUTTON)))
                .response
                .on_hover_text("No future selections found");
        }

        None
    }
}

fn selection_index_ui(ui: &mut egui::Ui, index: usize) {
    ui.weak(RichText::new(format!("{index:3}")).monospace());
}

// Different kinds of selections can share the same path in practice! We need to
// differentiate those in the UI to avoid confusion.
fn selection_kind_ui(ui: &mut egui::Ui, sel: &Selection) {
    ui.weak(RichText::new(match sel {
        Selection::MsgId(_) => "(msg)",
        Selection::Instance(_) => "(instance)",
        Selection::DataPath(_) => "(field)",
        Selection::SpaceView(_) => "(view)",
        Selection::SpaceViewObjPath(_, _) => "(obj)",
        Selection::DataBlueprintGroup(_, _) => "(group)",
    }));
}

fn multi_selection_to_string(blueprint: &Blueprint, sel: &MultiSelection) -> String {
    if sel.selected().len() == 1 {
        single_selection_to_string(blueprint, &sel.selected().first().unwrap())
    } else {
        "<multiple objects>".to_owned()
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
        Selection::SpaceViewObjPath(_, obj_path) => obj_path.to_string(),
        Selection::DataBlueprintGroup(sid, handle) => {
            if let Some(space_view) = blueprint.viewport.space_view(sid) {
                if let Some(group) = space_view.data_blueprint.get_group(*handle) {
                    group.display_name.clone()
                } else {
                    format!("<removed group in {}>", space_view.name)
                }
            } else {
                "<group in removed space view>".to_owned()
            }
        }
        Selection::MsgId(s) => s.to_string(),
        Selection::Instance(s) => s.to_string(),
        Selection::DataPath(s) => s.to_string(),
    }
}

// TODO(cmc): This is both ad-hoc and technically incorrect: we should be using egui's
// `TextWrapping` job after patching it so that it can "wrap from the front".
fn selection_to_clipped_string(
    ui: &mut egui::Ui,
    blueprint: &Blueprint,
    sel: &MultiSelection,
    font_id: &egui::FontId,
    width: f32,
) -> String {
    let mut width = width - ui.fonts().glyph_width(font_id, '…');
    let mut sel = multi_selection_to_string(blueprint, sel)
        .chars()
        .rev()
        .take_while(|c| {
            width -= ui.fonts().glyph_width(font_id, *c);
            width > 0.0
        })
        .collect::<String>();
    if width <= 0.0 {
        sel += "…";
    }
    sel.chars().rev().collect()
}
