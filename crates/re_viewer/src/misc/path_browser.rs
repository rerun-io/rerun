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

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct SelectionHistory {
    current: usize, // index into `self.stack`
    stack: Vec<Selection>,
    show_detailed: bool,
}

impl SelectionHistory {
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
        let prev_next = ui
            .horizontal(|ui| {
                let prev = self.show_prev_button(ui);

                let picked = egui::ComboBox::from_id_source("history_browser")
                    .width(ui.available_width() * 0.5)
                    .wrap(false)
                    .selected_text(
                        self.current()
                            .map(|sel| selection_to_string(&sel.selection).unwrap())
                            .unwrap_or_else(|| String::new()),
                    )
                    .show_ui(ui, |ui| {
                        for (i, sel) in self.stack.iter().enumerate() {
                            ui.horizontal(|ui| {
                                let index_str = egui::RichText::new(format!("[{i}]")).monospace();
                                ui.weak(index_str);
                                let str = selection_to_string(sel).unwrap();
                                ui.selectable_value(&mut self.current, i, str);
                            });
                        }
                    })
                    .inner
                    .and_then(|_| self.current());

                let shortcut = &crate::ui::kb_shortcuts::SELECTION_DETAILED;
                if ui
                    .small_button(if self.show_detailed { "⏷" } else { "⏶" })
                    .on_hover_text(format!(
                        "{} detailed history view ({})",
                        if self.show_detailed { "Collapsed" } else { "Expand" },
                        ui.ctx().format_shortcut(shortcut)
                    ))
                    .clicked()
                    // TODO(cmc): feels like using the shortcut should highlight the associated
                    // button or something.
                    || ui.ctx().input_mut().consume_shortcut(shortcut)
                {
                    self.show_detailed = !self.show_detailed;
                }

                let next = self.show_next_button(ui);

                prev.or(picked).or(next)
            })
            .inner;

        if self.show_detailed {
            return prev_next;
        }

        let new_selection = ui
            .vertical(|ui| {
                let mut new_selection = None;

                fn show_row(
                    ui: &mut egui::Ui,
                    enabled: bool,
                    label: &str,
                    sel: Option<HistoricalSelection>,
                ) -> bool {
                    ui.label(label);

                    let Some(sel) = sel else {
                        ui.end_row();
                        return false;
                    };

                    let clicked = ui
                        .add_enabled_ui(enabled, |ui| {
                            ui.horizontal(|ui| {
                                let index_str =
                                    egui::RichText::new(format!("[{}]", sel.index)).monospace();
                                ui.weak(index_str);
                                let path = selection_to_string(&sel.selection).unwrap();
                                ui.selectable_label(false, path).clicked()
                            })
                            .inner
                        })
                        .inner;

                    ui.end_row();

                    clicked
                }

                egui::Grid::new("selection_history")
                    .num_columns(3)
                    .show(ui, |ui| {
                        if show_row(ui, true, "Previous", self.previous()) {
                            self.current -= 1;
                            new_selection = self.current();
                        }

                        _ = show_row(ui, false, "Current", self.current());

                        if show_row(ui, true, "Next", self.next()) {
                            self.current += 1;
                            new_selection = self.current();
                        }
                    });

                new_selection
            })
            .inner;

        prev_next.or(new_selection)
    }

    fn show_prev_button(&mut self, ui: &mut egui::Ui) -> Option<HistoricalSelection> {
        const PREV_BUTTON: &str = "⏴ Prev";
        if let Some(previous) = self.previous() {
            let shortcut = &crate::ui::kb_shortcuts::SELECTION_PREVIOUS;
            if ui
                .small_button(PREV_BUTTON)
                .on_hover_text(format!(
                    "Go to previous selection ({}):\n[{}] {}",
                    ui.ctx().format_shortcut(shortcut),
                    previous.index,
                    selection_to_string(&previous.selection).unwrap()
                ))
                .clicked()
                // TODO(cmc): feels like using the shortcut should highlight the associated
                // button or something.
                || ui.ctx().input_mut().consume_shortcut(shortcut)
            {
                if previous.index != self.current {
                    self.current = previous.index;
                    return self.current();
                }
            }
        } else {
            // Creating a superfluous horizontal UI so that we can still have hover text.
            ui.horizontal(|ui| ui.add_enabled(false, egui::Button::new(PREV_BUTTON)))
                .response
                .on_hover_text("No past selections found");
        }

        None
    }

    fn show_next_button(&mut self, ui: &mut egui::Ui) -> Option<HistoricalSelection> {
        const NEXT_BUTTON: &str = "Next ⏵";
        if let Some(next) = self.next() {
            let shortcut = &crate::ui::kb_shortcuts::SELECTION_NEXT;
            if ui
                .small_button(NEXT_BUTTON)
                .on_hover_text(format!(
                    "Go to next selection ({}):\n[{}] {}",
                    ui.ctx().format_shortcut(shortcut),
                    next.index,
                    selection_to_string(&next.selection).unwrap()
                ))
                .clicked()
                // TODO(cmc): feels like using the shortcut should highlight the associated
                // button or something.
                || ui.ctx().input_mut().consume_shortcut(shortcut)
            {
                if next.index != self.current {
                    self.current = next.index;
                    return self.current();
                }
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
