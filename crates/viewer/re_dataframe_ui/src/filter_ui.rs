use std::sync::Arc;

use egui::{Frame, Margin, Style, text::LayoutJob};

use re_ui::{SyntaxHighlighting, UiExt as _, syntax_highlighting::SyntaxHighlightedBuilder};

use crate::TableBlueprint;
use crate::filters::{Filter, FilterOperation};

/// Current state of the filter bar.
///
/// Since this is dynamically changed, e.g. as the user types a query, the content of [`Self`] can
/// differ from the content of [`TableBlueprint::filters`]. [`Self::filter_bar_ui`] returns a flag
/// to indicate when this content should be committed to the blueprint.
#[derive(Clone, Debug)]
pub struct FilterState {
    pub filters: Vec<IdentifiedFilter>,
    pub active_filter: Option<usize>,
}

impl FilterState {
    /// Restore the saved state, initializing it from the blueprint if needed.
    ///
    /// Call this at the beginning of the frame.
    pub fn load_or_init_from_blueprint(
        ctx: &egui::Context,
        persisted_id: egui::Id,
        table_blueprint: &TableBlueprint,
    ) -> Self {
        ctx.data_mut(|data| {
            data.get_temp_mut_or_insert_with(persisted_id, || Self {
                filters: table_blueprint
                    .filters
                    .iter()
                    .cloned()
                    .map(Into::into)
                    .collect(),
                active_filter: None,
            })
            .clone()
        })
    }

    /// Store the state to the temporary memory.
    ///
    /// Call this at the end of the frame.
    pub fn store(self, ctx: &egui::Context, persisted_id: egui::Id) {
        ctx.data_mut(|data| {
            data.insert_temp(persisted_id, self);
        });
    }

    /// Add a new filter to the filter bar.
    pub fn push_new_filter(&mut self, filter: Filter) {
        self.filters.push(filter.into());
        self.active_filter = Some(self.filters.len() - 1);
    }

    /// Convert the current state into filters to be used by the table blueprint.
    pub fn to_blueprint_filters(&self) -> Vec<Filter> {
        self.filters.iter().map(|f| f.filter.clone()).collect()
    }

    /// Display the filter bar UI.
    ///
    /// Returns true if the filter must be committed.
    #[must_use]
    pub fn filter_bar_ui(&mut self, ui: &mut egui::Ui) -> bool {
        if self.filters.is_empty() {
            return false;
        }

        let mut should_commit = false;

        Frame::new()
            .inner_margin(Margin {
                top: 16,
                bottom: 12,
                left: 16,
                right: 16,
            })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let active_index = self.active_filter.take();

                    let mut remove_idx = None;
                    for (index, filter) in self.filters.iter_mut().enumerate() {
                        let result = filter.ui(ui, Some(index) == active_index);
                        should_commit |= result.should_commit;
                        if result.should_close {
                            remove_idx = Some(index);
                        }
                    }

                    if let Some(remove_idx) = remove_idx {
                        self.active_filter = None;
                        self.filters.remove(remove_idx);
                        should_commit = true;
                    }
                });
            });

        should_commit
    }
}

/// Output of the `DisplayFilter::ui` method.
struct DisplayFilterUiResult {
    should_commit: bool,
    should_close: bool,
}

/// Wrapper over [`Filter`] with an associated id.
#[derive(Clone, Debug)]
pub struct IdentifiedFilter {
    filter: Filter,

    /// Unique id of this filter, randomly generated upon creation.
    id: egui::Id,
}

impl From<Filter> for IdentifiedFilter {
    fn from(value: Filter) -> Self {
        let id = egui::Id::new(getrandom::u64().expect("Failed to generate a random id"));

        Self { filter: value, id }
    }
}

impl IdentifiedFilter {
    /// UI for a single filter.
    ///
    /// Returns true if the filter must be committed.
    #[must_use]
    fn ui(&mut self, ui: &mut egui::Ui, activate_filter: bool) -> DisplayFilterUiResult {
        let mut result = DisplayFilterUiResult {
            should_commit: false,
            should_close: false,
        };

        let mut response = Frame::new()
            .inner_margin(Margin::symmetric(4, 4))
            .stroke(ui.tokens().table_filter_frame_stroke)
            .corner_radius(2.0)
            .show(ui, |ui| {
                let widget_text = SyntaxHighlightedBuilder::new(Arc::clone(ui.style()))
                    .append(&self.filter.column_name)
                    .append(&" ")
                    .append(&SyntaxHighlightFilterOperation {
                        ui,
                        filter_operation: &self.filter.operation,
                    })
                    .into_widget_text();

                let text_response = ui.add(
                    egui::Label::new(widget_text)
                        .selectable(false)
                        .sense(egui::Sense::click()),
                );

                if ui
                    .small_icon_button(&re_ui::icons::CLOSE, "Remove filter")
                    .clicked()
                {
                    result.should_close = true;
                }

                text_response
            });

        let popup_was_closed = !egui::Popup::is_id_open(ui.ctx(), self.id);

        response.inner.interact_rect = response.response.interact_rect.expand(3.0);
        let mut popup = egui::Popup::menu(&response.inner)
            .id(self.id)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);

        if activate_filter {
            popup = popup.open_memory(Some(egui::SetOpenCommand::Bool(true)));
        }

        let popup_response = popup.show(|ui| {
            self.filter.operation.popup_ui(ui, popup_was_closed);
        });

        if popup_response.is_some_and(|inner_response| inner_response.response.should_close()) {
            result.should_commit = true;
        }

        result
    }
}

// TODO(#11059): this helper is only needed because the `SyntaxHighlighting` trait has no access to
// the current theme, so it cannot access design tokens.
struct SyntaxHighlightFilterOperation<'a> {
    ui: &'a mut egui::Ui,
    filter_operation: &'a FilterOperation,
}

impl SyntaxHighlighting for SyntaxHighlightFilterOperation<'_> {
    fn syntax_highlight_into(&self, style: &Style, job: &mut LayoutJob) {
        let normal_text_format = egui::TextFormat::simple(
            egui::TextStyle::Body.resolve(style),
            egui::Color32::PLACEHOLDER,
        );
        let operator_text_format = egui::TextFormat::simple(
            egui::TextStyle::Body.resolve(style),
            self.ui.tokens().table_filter_operator_text_color,
        );
        let rhs_text_format = egui::TextFormat::simple(
            egui::TextStyle::Body.resolve(style),
            self.ui.tokens().table_filter_rhs_text_color,
        );

        job.append(
            self.filter_operation.operator_text(),
            0.0,
            operator_text_format,
        );

        job.append(" ", 0.0, normal_text_format.clone());
        let rhs_text = self.filter_operation.rhs_text();
        job.append(
            if rhs_text.is_empty() {
                "…"
            } else {
                &rhs_text
            },
            0.0,
            rhs_text_format,
        );
    }
}

impl FilterOperation {
    /// Returns true if the filter must be committed.
    fn popup_ui(&mut self, ui: &mut egui::Ui, popup_just_opened: bool) -> bool {
        let mut should_commit = false;

        match self {
            Self::StringContains(query) => {
                ui.label("contains");
                let response = ui.text_edit_singleline(query);
                if popup_just_opened {
                    response.request_focus();
                }

                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    ui.close();
                    should_commit = true;
                }
            }
        }

        should_commit
    }

    /// Display text of the operator.
    fn operator_text(&self) -> &'static str {
        match self {
            Self::StringContains(_) => "contains",
        }
    }

    /// Display text of the right-hand side operand (aka the user-provided query data).
    fn rhs_text(&self) -> String {
        match self {
            Self::StringContains(query) => query.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_ui() {
        // Let's remember to update this test when adding new filter operations.
        let _: () = {
            let _op = FilterOperation::StringContains(String::new());
            match _op {
                FilterOperation::StringContains(_) => {}
            }
        };

        let test_cases = [
            (
                FilterOperation::StringContains("query".to_owned()),
                "string_contains",
            ),
            (
                FilterOperation::StringContains(String::new()),
                "string_contains_empty",
            ),
        ];

        for (filter_op, test_name) in test_cases {
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::Vec2::new(500.0, 80.0))
                .build_ui(|ui| {
                    re_ui::apply_style_and_install_loaders(ui.ctx());

                    let mut filter_state = FilterState {
                        filters: vec![
                            Filter::new("column:name".to_owned(), filter_op.clone()).into(),
                        ],
                        active_filter: None,
                    };

                    let _res = filter_state.filter_bar_ui(ui);
                });

            harness.run();

            harness.snapshot(format!("filter_ui_{test_name}"));
        }
    }
}
