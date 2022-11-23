use std::sync::Arc;

use re_log_types::{
    path::{IndexPath, ObjPathImpl, ObjTypePath},
    ObjPathComp,
};

use super::{Selection, ViewerContext};

// ---

#[derive(Debug, Clone)]
pub struct HistoricalSelection {
    pub index: usize,
    pub selection: Selection,
}

impl From<(usize, Selection)> for HistoricalSelection {
    fn from((index, selection): (usize, Selection)) -> Self {
        Self { index, selection }
    }
}

// ---

// TODO:
// - goto previous: go backwards in the stack, don't remove anything
// - goto next: go forwards in the stack, don't add anything
// - goto parent: go to parent "directory", clear stack upwards, add to the stack
// - goto <clicked>: go to <clicked> "directory", clear stack upwards, add to the stack
//
// TODO:
// - menu edit > undo/redo selection
// - rolling list of history

// TODO: so more like a selection browser, really.
#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PathBrowser {
    current: usize, // index into `self.stack`
    stack: Vec<Selection>,
}

impl PathBrowser {
    pub fn current(&self) -> Option<HistoricalSelection> {
        self.stack
            .get(self.current)
            .cloned()
            .map(|s| (self.current, s).into())
    }
    pub fn previous(&self) -> Option<HistoricalSelection> {
        (self.current > 0).then(|| (self.current - 1, self.stack[self.current - 1].clone()).into())
    }
    pub fn next(&self) -> Option<HistoricalSelection> {
        (self.current < self.stack.len().saturating_sub(1))
            .then(|| (self.current + 1, self.stack[self.current + 1].clone()).into())
    }

    pub fn select(&mut self, selection: &Selection) {
        if matches!(
            selection,
            Selection::ObjTypePath(_)
                | Selection::Instance(_)
                | Selection::DataPath(_)
                | Selection::Space(_)
        ) {
            if let Some(current) = self.current() {
                if current.selection == *selection {
                    return;
                }
            }

            if !self.stack.is_empty() {
                self.stack.drain(self.current + 1..);
            }
            self.stack.push(selection.clone());
            self.current = self.stack.len() - 1;
        }
    }

    pub(crate) fn show(
        &mut self,
        ctx: &mut ViewerContext,
        ui: &mut egui::Ui,
    ) -> Option<HistoricalSelection> {
        ui.vertical(|ui| {
            let mut new_selection = None;

            // TODO: show scrollable combobox with everything

            egui::Grid::new("selection_history").show(ui, |ui| {
                ui.label("Current");
                ui.horizontal(|ui| {
                    if let Some(current) = self.current() {
                        ui.weak(format!("[{}]", current.index));
                        let path = selection_to_string(&current.selection).unwrap();
                        _ = ui.selectable_label(false, path);
                    }
                });
                ui.end_row();

                ui.label("Previous");
                ui.horizontal(|ui| {
                    if let Some(previous) = self.previous() {
                        ui.weak(format!("[{}]", previous.index));
                        let path = selection_to_string(&previous.selection).unwrap();
                        if ui.selectable_label(false, path).clicked() {
                            self.current = previous.index;
                            new_selection = self.current();
                        }
                    }
                });
                ui.end_row();

                ui.label("Next");
                ui.horizontal(|ui| {
                    if let Some(next) = self.next() {
                        ui.weak(format!("[{}]", next.index));
                        let path = selection_to_string(&next.selection).unwrap();
                        if ui.selectable_label(false, path).clicked() {
                            self.current = next.index;
                            new_selection = self.current();
                        }
                    }
                });
                ui.end_row();
            });

            new_selection
        })
        .inner
    }

    fn show_prev_button(&mut self, ui: &mut egui::Ui) -> Option<HistoricalSelection> {
        if let Some(previous) = self.previous() {
            if ui
                .small_button("⏴")
                .on_hover_text(format!(
                    "Go to previous selection ({})\nThis will take you back to: [{}] {}",
                    ui.ctx()
                        .format_shortcut(&crate::ui::kb_shortcuts::SELECTION_PREVIOUS),
                    previous.index,
                    selection_to_string(&previous.selection).unwrap()
                ))
                .clicked()
            {
                if previous.index != self.current {
                    self.current = previous.index;
                    return self.current();
                }
            }
        } else {
            // Creating a superfluous horizontal UI so that we can still have hover text.
            ui.horizontal(|ui| ui.add_enabled(false, egui::Button::new("⏴")))
                .response
                .on_hover_text("No past selections found");
        }

        None
    }

    fn show_next_button(&mut self, ui: &mut egui::Ui) -> Option<HistoricalSelection> {
        if let Some(next) = self.next() {
            if ui
                .small_button("⏴")
                .on_hover_text(format!(
                    "Go to next selection ({})\nThis will take you back to: [{}] {}",
                    ui.ctx()
                        .format_shortcut(&crate::ui::kb_shortcuts::SELECTION_NEXT),
                    next.index,
                    selection_to_string(&next.selection).unwrap()
                ))
                .clicked()
            {
                if next.index != self.current {
                    self.current = next.index;
                    return self.current();
                }
            }
        } else {
            // Creating a superfluous horizontal UI so that we can still have hover text.
            ui.horizontal(|ui| ui.add_enabled(false, egui::Button::new("⏴")))
                .response
                .on_hover_text("No past selections found");
        }

        None
    }
}

fn selection_to_string(selection: &Selection) -> Option<String> {
    match selection {
        Selection::ObjTypePath(path) => path.to_string(),
        Selection::Instance(path) => path.to_string(),
        Selection::DataPath(path) => path.to_string(),
        Selection::Space(path) => path.to_string(),
        _ => return None,
    }
    .into()
}
