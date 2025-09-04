use egui::{Frame, Margin};

use re_ui::{
    SyntaxHighlighting, UiExt as _, design_tokens_of_visuals,
    syntax_highlighting::SyntaxHighlightedBuilder,
};

use crate::TableBlueprint;
use crate::filters::{Filter, FilterOperation};

/// Action to take based on the user interaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum FilterUiAction {
    #[default]
    None,

    /// The user as closed the filter popup using enter or clicking outside. The updated filter
    /// state should be committed to the table blueprint.
    CommitStateToBlueprint,

    /// The user closed the filter popup using escape, so the edit should be cancelled by resetting
    /// the filter state from the table blueprint.
    CancelStateEdit,
}

impl FilterUiAction {
    fn merge(self, other: Self) -> Self {
        // We only consider the first non-noop action. There should never be more than one in a
        // frame anyway.
        match (self, other) {
            (Self::None, other) => other,
            (Self::CommitStateToBlueprint | Self::CancelStateEdit, _) => self,
        }
    }
}

/// Current state of the filter bar.
///
/// Since this is dynamically changed, e.g. as the user types a query, the content of [`Self`] can
/// differ from the content of [`TableBlueprint::filters`]. [`Self::filter_bar_ui`] returns a flag
/// to indicate when this content should be committed to the blueprint.
#[derive(Clone, Debug)]
pub struct FilterState {
    pub filters: Vec<Filter>,
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
                filters: table_blueprint.filters.clone(),
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
        self.filters.push(filter);
        self.active_filter = Some(self.filters.len() - 1);
    }

    /// Display the filter bar UI.
    ///
    /// This handles committing and/or restoring the state from the blueprint.
    pub fn filter_bar_ui(&mut self, ui: &mut egui::Ui, table_blueprint: &mut TableBlueprint) {
        let action = self.filter_bar_ui_impl(ui);

        match action {
            FilterUiAction::None => {}

            FilterUiAction::CommitStateToBlueprint => {
                table_blueprint.filters = self.filters.clone();
            }

            FilterUiAction::CancelStateEdit => {
                self.filters = table_blueprint.filters.clone();
                self.active_filter = None;
            }
        }
    }

    #[must_use]
    fn filter_bar_ui_impl(&mut self, ui: &mut egui::Ui) -> FilterUiAction {
        if self.filters.is_empty() {
            return Default::default();
        }

        let mut should_commit = false;
        let mut action = FilterUiAction::None;

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
                        let filter_id = ui.make_persistent_id(index);
                        let result = filter.ui(ui, filter_id, Some(index) == active_index);

                        action = action.merge(result.filter_action);

                        if result.should_delete_filter {
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

        action
    }
}

/// Output of the `DisplayFilter::ui` method.
struct DisplayFilterUiResult {
    filter_action: FilterUiAction,
    should_delete_filter: bool,
}

impl Filter {
    /// UI for a single filter.
    #[must_use]
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        filter_id: egui::Id,
        activate_filter: bool,
    ) -> DisplayFilterUiResult {
        let mut should_delete_filter = false;
        let mut action_due_to_filter_deletion = FilterUiAction::None;

        let mut response = Frame::new()
            .inner_margin(Margin::symmetric(4, 4))
            .stroke(ui.tokens().table_filter_frame_stroke)
            .corner_radius(2.0)
            .show(ui, |ui| {
                let widget_text = SyntaxHighlightedBuilder::new(ui.style())
                    .with(&self.column_name)
                    .with(&" ")
                    .with(&self.operation)
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
                    should_delete_filter = true;
                    action_due_to_filter_deletion = FilterUiAction::CommitStateToBlueprint;
                }

                text_response
            });

        let popup_was_closed = !egui::Popup::is_id_open(ui.ctx(), filter_id);

        response.inner.interact_rect = response.response.interact_rect.expand(3.0);
        let mut popup = egui::Popup::menu(&response.inner)
            .id(filter_id)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);

        if activate_filter {
            popup = popup.open_memory(Some(egui::SetOpenCommand::Bool(true)));
        }

        let popup_response = popup.show(|ui| {
            let action = self
                .operation
                .popup_ui(ui, self.column_name.as_ref(), popup_was_closed);

            // Ensure we close the popup if the popup ui decided on an action.
            if action != FilterUiAction::None {
                ui.close();
            }

            action
        });

        // Handle the logic of committing or cancelling the filter edit. This can happen in three
        // ways:
        //
        // 1) A filter was deleted. This triggers a commit.
        // 2) The popup is closed by "normal" means (e.g. clicking outside, etc.). This triggers a
        //    commit, unless it happened with Esc, in which case we cancel the edit.
        // 3) The `FilterOperation::popup_ui` itself triggers a commit/cancel action (typically
        //    when interacting with a text field and detecting either Enter or Esc). When that
        //    happens, we close the popup and propagate the action.

        let (action_due_to_closed_popup, action_from_popup_ui) = popup_response
            .map(|inner_response| {
                let action_due_to_closed_popup = if inner_response.response.should_close() {
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        FilterUiAction::CancelStateEdit
                    } else {
                        FilterUiAction::CommitStateToBlueprint
                    }
                } else {
                    FilterUiAction::None
                };

                (action_due_to_closed_popup, inner_response.inner)
            })
            .unwrap_or_default();

        let filter_action = action_due_to_filter_deletion
            .merge(action_due_to_closed_popup)
            .merge(action_from_popup_ui);

        DisplayFilterUiResult {
            filter_action,
            should_delete_filter,
        }
    }
}

impl SyntaxHighlighting for FilterOperation {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder<'_>) {
        let normal_text_format = |style| {
            egui::TextFormat::simple(
                egui::TextStyle::Body.resolve(style),
                egui::Color32::PLACEHOLDER,
            )
        };
        let operator_text_format = |style| {
            egui::TextFormat::simple(
                egui::TextStyle::Body.resolve(style),
                design_tokens_of_visuals(&style.visuals).table_filter_operator_text_color,
            )
        };
        let rhs_text_format = |style| {
            egui::TextFormat::simple(
                egui::TextStyle::Body.resolve(style),
                design_tokens_of_visuals(&style.visuals).table_filter_rhs_text_color,
            )
        };

        builder.append_with_format(self.operator_text(), operator_text_format);

        builder.append_with_format(" ", normal_text_format);
        let rhs_text = self.rhs_text();
        builder.append_with_format(
            if rhs_text.is_empty() {
                "…"
            } else {
                &rhs_text
            },
            rhs_text_format,
        );
    }
}

impl FilterOperation {
    /// Returns true if the filter must be committed.
    fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        column_name: &str,
        popup_just_opened: bool,
    ) -> FilterUiAction {
        let mut action = FilterUiAction::None;

        let mut top_text_builder = SyntaxHighlightedBuilder::new(ui.style());
        top_text_builder.append(&column_name);
        top_text_builder.append(&" ");
        top_text_builder.append_primitive(self.operator_text());
        let top_text = top_text_builder.into_widget_text();

        match self {
            Self::StringContains(query) => {
                ui.label(top_text);
                let response = ui.text_edit_singleline(query);
                if popup_just_opened {
                    response.request_focus();
                }

                if response.lost_focus() {
                    action = ui.input(|i| {
                        if i.key_pressed(egui::Key::Enter) {
                            FilterUiAction::CommitStateToBlueprint
                        } else if i.key_pressed(egui::Key::Escape) {
                            FilterUiAction::CancelStateEdit
                        } else {
                            FilterUiAction::None
                        }
                    });
                }
            }

            Self::BooleanEquals(query) => {
                ui.label(top_text);
                if ui.re_radio_value(query, true, "true").clicked()
                    || ui.re_radio_value(query, false, "false").clicked()
                {
                    action = FilterUiAction::CommitStateToBlueprint;
                }
            }
        }

        action
    }

    /// Display text of the operator.
    fn operator_text(&self) -> &'static str {
        match self {
            Self::StringContains(_) => "contains",

            Self::BooleanEquals(_) => "is",
        }
    }

    /// Display text of the right-hand side operand (aka the user-provided query data).
    fn rhs_text(&self) -> String {
        match self {
            Self::StringContains(query) => query.clone(),

            Self::BooleanEquals(query) => format!("{query}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cases() -> Vec<(FilterOperation, &'static str)> {
        // Let's remember to update this test when adding new filter operations.
        #[cfg(debug_assertions)]
        let _: () = {
            let _op = FilterOperation::StringContains(String::new());
            match _op {
                FilterOperation::StringContains(_) | FilterOperation::BooleanEquals(_) => {}
            }
        };

        [
            (
                FilterOperation::StringContains("query".to_owned()),
                "string_contains",
            ),
            (
                FilterOperation::StringContains(String::new()),
                "string_contains_empty",
            ),
            (FilterOperation::BooleanEquals(true), "boolean_equals_true"),
            (
                FilterOperation::BooleanEquals(false),
                "boolean_equals_false",
            ),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn test_filter_ui() {
        for (filter_op, test_name) in test_cases() {
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::Vec2::new(500.0, 80.0))
                .build_ui(|ui| {
                    re_ui::apply_style_and_install_loaders(ui.ctx());

                    let mut filter_state = FilterState {
                        filters: vec![Filter::new("column:name".to_owned(), filter_op.clone())],
                        active_filter: None,
                    };

                    let _res = filter_state.filter_bar_ui_impl(ui);
                });

            harness.run();

            harness.snapshot(format!("filter_ui_{test_name}"));
        }
    }

    #[test]
    fn test_popup_ui() {
        for (mut filter_op, test_name) in test_cases() {
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::Vec2::new(700.0, 500.0))
                .build_ui(|ui| {
                    re_ui::apply_style_and_install_loaders(ui.ctx());

                    let _res = filter_op.popup_ui(ui, "column:name", true);
                });

            harness.run();

            harness.snapshot(format!("popup_ui_{test_name}"));
        }
    }
}
