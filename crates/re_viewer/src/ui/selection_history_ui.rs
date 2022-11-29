use egui::RichText;

use super::{HistoricalSelection, SelectionHistory};
use crate::{ui::Blueprint, Selection};

// ---

impl SelectionHistory {
    pub(crate) fn selection_ui(
        &mut self,
        ui: &mut egui::Ui,
        blueprint: &Blueprint,
    ) -> Option<Selection> {
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

        const BIG_BUTTON_SIZE: f32 = 50.0;
        const MIN_COMBOBOX_SIZE: f32 = 100.0;

        let font_id = egui::TextStyle::Body.resolve(ui.style());

        let mut res = None;
        StripBuilder::new(ui)
            .size(Size::exact(BIG_BUTTON_SIZE)) // prev
            .size(Size::remainder().at_least(MIN_COMBOBOX_SIZE)) // browser
            .size(Size::exact(BIG_BUTTON_SIZE)) // next
            .horizontal(|mut strip| {
                // prev
                let mut prev = None;
                strip.cell(|ui| {
                    prev = self.prev_button_ui(ui, blueprint);
                });

                // browser
                let mut picked = None;
                strip.cell(|ui| {
                    let clipped_width = ui.available_width() - 20.0; // leave some space for the icon!
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
                                    show_selection_index(ui, i);
                                    {
                                        // borrow checker workaround
                                        let sel = selection_to_clipped_string(
                                            ui,
                                            blueprint,
                                            sel,
                                            &font_id,
                                            clipped_width,
                                        );
                                        ui.selectable_value(&mut self.current, i, sel);
                                    }
                                    show_selection_kind(ui, sel);
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

    fn prev_button_ui(
        &mut self,
        ui: &mut egui::Ui,
        blueprint: &Blueprint,
    ) -> Option<HistoricalSelection> {
        const PREV_BUTTON: &str = "⏴ Prev";
        if let Some(previous) = self.previous() {
            let shortcut = &crate::ui::kb_shortcuts::SELECTION_PREVIOUS;
            let button_clicked = ui
                .small_button(PREV_BUTTON)
                .on_hover_text(format!(
                    "Go to previous selection ({}):\n[{}] {}",
                    ui.ctx().format_shortcut(shortcut),
                    previous.index,
                    selection_to_string(blueprint, &previous.selection),
                ))
                .clicked();
            // TODO(cmc): feels like using the shortcut should highlight the associated
            // button or something (but then again it, it'd make more sense to do that
            // at the egui level rather than specifically here).
            let shortcut_used = ui.ctx().input_mut().consume_shortcut(shortcut);
            if (button_clicked || shortcut_used) && previous.index != self.current {
                self.current = previous.index;
                return self.current();
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
        const NEXT_BUTTON: &str = "Next ⏵";
        if let Some(next) = self.next() {
            let shortcut = &crate::ui::kb_shortcuts::SELECTION_NEXT;
            let button_clicked = ui
                .small_button(NEXT_BUTTON)
                .on_hover_text(format!(
                    "Go to next selection ({}):\n[{}] {}",
                    ui.ctx().format_shortcut(shortcut),
                    next.index,
                    selection_to_string(blueprint, &next.selection),
                ))
                .clicked();
            // TODO(cmc): feels like using the shortcut should highlight the associated
            // button or something (but then again it, it'd make more sense to do that
            // at the egui level rather than specifically here).
            let shortcut_used = ui.ctx().input_mut().consume_shortcut(shortcut);
            if (button_clicked || shortcut_used) && next.index != self.current {
                self.current = next.index;
                return self.current();
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

fn show_selection_index(ui: &mut egui::Ui, index: usize) {
    ui.weak(RichText::new(format!("{index:3}")).monospace());
}

// Different kinds of selections can share the same path in practice! We need to
// differentiate those in the UI to avoid confusion.
fn show_selection_kind(ui: &mut egui::Ui, sel: &Selection) {
    ui.weak(RichText::new(match sel {
        Selection::None => "(none)",
        Selection::MsgId(_) => "(msg)",
        Selection::ObjTypePath(_) => "(type)",
        Selection::Instance(_) => "(instance)",
        Selection::DataPath(_) => "(field)",
        Selection::Space(_) => "(space)",
        Selection::SpaceView(_) => "(view)",
        Selection::SpaceViewObjPath(_, _) => "(obj)",
    }));
}

fn selection_to_string(blueprint: &Blueprint, sel: &Selection) -> String {
    match sel {
        Selection::SpaceView(sid) => {
            if let Some(space_view) = blueprint.viewport.space_view(sid) {
                return space_view.name.clone();
            }
        }
        Selection::SpaceViewObjPath(_, obj_path) => return obj_path.to_string(),
        _ => {}
    }

    sel.to_string()
}

// TODO(cmc): This is both ad-hoc and technically incorrect: we should be using egui's
// `TextWrapping` job after patching it so that it can "wrap from the front".
fn selection_to_clipped_string(
    ui: &mut egui::Ui,
    blueprint: &Blueprint,
    sel: &Selection,
    font_id: &egui::FontId,
    width: f32,
) -> String {
    let mut width = width - ui.fonts().glyph_width(font_id, '…');
    let mut sel = selection_to_string(blueprint, sel)
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
