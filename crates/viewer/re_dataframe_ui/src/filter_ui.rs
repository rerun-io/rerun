use re_ui::UiExt as _;

use crate::TableBlueprint;
use crate::filters::{Filter, FilterOperation};

#[derive(Clone, Debug)]
pub struct FilterState {
    pub filters: Vec<Filter>,
    pub active_filter: Option<usize>,
}

impl FilterState {
    pub fn get_from_blueprint(
        ctx: &egui::Context,
        persisted_id: egui::Id,
        table_blueprint: &TableBlueprint,
    ) -> Self {
        ctx.data_mut(|data| {
            data.get_temp_mut_or_insert_with(persisted_id, || Self {
                filters: table_blueprint.filters.clone(),
                active_filter: None,
            })
            .clone()
        })
    }

    pub fn store(self, ctx: &egui::Context, persisted_id: egui::Id) {
        ctx.data_mut(|data| {
            data.insert_temp(persisted_id, self);
        });
    }

    pub fn push_new_filter(&mut self, filter: Filter) {
        self.filters.push(filter);
        self.active_filter = Some(self.filters.len() - 1);
    }

    /// Returns true if the filter must be committed.
    pub fn filter_bar_ui(&mut self, ui: &mut egui::Ui) -> bool {
        if self.filters.is_empty() {
            return false;
        }

        let mut should_commit = false;

        ui.horizontal(|ui| {
            let active_index = self.active_filter.take();

            let mut remove_idx = None;
            for (index, filter) in self.filters.iter_mut().enumerate() {
                should_commit |= filter.ui(ui, Some(index) == active_index);
                if ui
                    .small_icon_button(&re_ui::icons::CLOSE, "Remove filter")
                    .clicked()
                {
                    remove_idx = Some(index);
                }
            }

            if let Some(remove_idx) = remove_idx {
                self.active_filter = None;
                self.filters.remove(remove_idx);
                should_commit = true;
            }
        });

        should_commit
    }
}

impl Filter {
    /// Returns true if the filter must be committed.
    fn ui(&mut self, ui: &mut egui::Ui, activate_filter: bool) -> bool {
        let mut should_commit = false;

        ui.label(&self.column_name);
        should_commit |= self.operation.ui(ui, activate_filter);

        should_commit
    }
}

impl FilterOperation {
    /// Returns true if the filter must be committed.
    fn ui(&mut self, ui: &mut egui::Ui, activate_filter: bool) -> bool {
        let mut should_commit = false;

        match self {
            Self::StringContains(query) => {
                ui.label("contains");
                let response = ui.text_edit_singleline(query);
                if activate_filter {
                    response.request_focus();
                }
                should_commit |= response.lost_focus();
            }
        }

        should_commit
    }
}
