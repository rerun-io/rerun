use ahash::HashSet;
use egui::{Id, Modifiers};

#[derive(Debug, Clone, Default)]
pub struct TableSelectionState {
    pub selected_rows: HashSet<u64>,

    /// Used as the "from" for shift-selections.
    pub last_non_shift_selected_row: Option<u64>,
    pub last_shift_selected_row: Option<u64>,

    pub hovered_row: Option<u64>,
    pub all_hovered: bool,
}

impl TableSelectionState {
    pub fn load(egui_ctx: &egui::Context, id: Id) -> Self {
        egui_ctx.data(|data| data.get_temp(id).unwrap_or_default())
    }

    pub fn store(self, egui_ctx: &egui::Context, id: Id) {
        egui_ctx.data_mut(|data| {
            data.insert_temp(id, self);
        });
    }

    pub fn clear(egui_ctx: &egui::Context, id: Id) {
        egui_ctx.data_mut(|data| {
            data.remove::<Self>(id);
        });
    }

    /// Update the selection based on a row / item click.
    ///
    /// `checkbox_click` will act as if [`Modifiers::COMMAND`] is held (so, toggle selection for
    /// that row without clearing other selections).
    pub fn handle_row_click(&mut self, row: u64, modifiers: Modifiers, checkbox_click: bool) {
        if modifiers.shift {
            if let Some(last_shift) = self.last_shift_selected_row {
                // Clear previous shift selection
                let (start, end) = if last_shift <= row {
                    (last_shift, row)
                } else {
                    (row, last_shift)
                };
                for r in start..=end {
                    self.selected_rows.remove(&r);
                }
            }
            if let Some(last) = self.last_non_shift_selected_row {
                let (start, end) = if last <= row {
                    (last, row)
                } else {
                    (row, last)
                };
                self.selected_rows.extend(start..=end);
            } else {
                self.selected_rows.insert(row);
            }
            self.last_shift_selected_row = Some(row);
        } else if modifiers.command || checkbox_click {
            if !self.selected_rows.remove(&row) {
                self.selected_rows.insert(row);
                self.last_non_shift_selected_row = Some(row);
            }
            self.last_shift_selected_row = None;
        } else {
            self.selected_rows.clear();
            self.selected_rows.insert(row);
            self.last_non_shift_selected_row = Some(row);
            self.last_shift_selected_row = None;
        }
    }
}
