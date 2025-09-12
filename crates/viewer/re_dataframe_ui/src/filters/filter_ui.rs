use std::sync::Arc;

use egui::{Frame, Margin};

use re_ui::{SyntaxHighlighting, UiExt as _, syntax_highlighting::SyntaxHighlightedBuilder};

use super::{ComparisonOperator, Filter, FilterOperation};
use crate::TableBlueprint;

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
                let active_index = self.active_filter.take();
                let mut remove_idx = None;

                // TODO(#11194): ideally, egui would allow wrapping `Frame` widget itself. Remove
                // this when it does.
                let prepared_uis = self
                    .filters
                    .iter()
                    .map(|filter| filter.prepare_ui(ui))
                    .collect::<Vec<_>>();
                let item_spacing = ui.style().spacing.item_spacing.x;
                let available_width = ui.available_width();
                let mut rows = vec1::vec1![vec![]];
                let mut current_left_position = 0.0;
                for (index, prepared_ui) in prepared_uis.iter().enumerate() {
                    if current_left_position > 0.0
                        && current_left_position + prepared_ui.desired_width() > available_width
                    {
                        rows.push(vec![]);
                        current_left_position = 0.0;
                    }

                    rows.last_mut().push(index);
                    current_left_position += prepared_ui.desired_width() + item_spacing;
                }

                for row in rows {
                    ui.horizontal(|ui| {
                        for index in row {
                            let filter_id = ui.make_persistent_id(index);
                            let filter = &mut self.filters[index];
                            let prepared_ui = &prepared_uis[index];

                            let result =
                                filter.ui(ui, prepared_ui, filter_id, Some(index) == active_index);

                            action = action.merge(result.filter_action);

                            if result.should_delete_filter {
                                remove_idx = Some(index);
                            }
                        }
                    });
                }

                if let Some(remove_idx) = remove_idx {
                    self.active_filter = None;
                    self.filters.remove(remove_idx);
                    should_commit = true;
                }
            });

        action
    }
}

/// Output of the `DisplayFilter::ui` method.
struct DisplayFilterUiResult {
    filter_action: FilterUiAction,
    should_delete_filter: bool,
}

// TODO(#11194): used by the manual wrapping code. Remove when no longer needed.
struct FilterPreparedUi {
    frame: Frame,
    galley: Arc<egui::Galley>,
    desired_width: f32,
}

impl FilterPreparedUi {
    fn desired_width(&self) -> f32 {
        self.desired_width
    }
}

impl Filter {
    /// Prepare the UI for this filter
    fn prepare_ui(&self, ui: &egui::Ui) -> FilterPreparedUi {
        let layout_job = SyntaxHighlightedBuilder::new()
            .with_body(&self.column_name)
            .with_keyword(" ")
            .with(&self.operation)
            .into_job(ui.style());

        let galley = ui.fonts(|f| f.layout_job(layout_job));

        let frame = Frame::new()
            .inner_margin(Margin::symmetric(4, 4))
            .stroke(ui.tokens().table_filter_frame_stroke)
            .corner_radius(2.0);

        let desired_width = galley.size().x
            + ui.style().spacing.item_spacing.x
            + ui.tokens().small_icon_size.x
            + frame.total_margin().sum().x;

        FilterPreparedUi {
            frame,
            galley,
            desired_width,
        }
    }

    /// UI for a single filter.
    #[must_use]
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        prepared_ui: &FilterPreparedUi,
        filter_id: egui::Id,
        activate_filter: bool,
    ) -> DisplayFilterUiResult {
        let mut should_delete_filter = false;
        let mut action_due_to_filter_deletion = FilterUiAction::None;

        let response = prepared_ui
            .frame
            .show(ui, |ui| {
                let text_response = ui.add(
                    egui::Label::new(Arc::clone(&prepared_ui.galley))
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
            })
            .inner;

        // Should the popup be open?
        //
        // Note: we currently manually handle the popup state to allow popup-in-popup UIs.
        //TODO(emilk/egui#7451): let egui handle that when popup-in-popup is supported.
        let mut popup_open: bool = ui.data(|data| data.get_temp(filter_id)).unwrap_or_default();
        let popup_was_closed = !popup_open;
        if activate_filter || response.clicked() {
            popup_open = true;
        }
        let any_popup_open = egui::Popup::is_any_open(ui.ctx());

        let popup = egui::Popup::menu(&response)
            .id(filter_id)
            .gap(3.0)
            .close_behavior(if any_popup_open {
                egui::PopupCloseBehavior::IgnoreClicks
            } else {
                egui::PopupCloseBehavior::CloseOnClickOutside
            })
            .open_bool(&mut popup_open);

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

        ui.data_mut(|data| data.insert_temp(filter_id, popup_open));

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
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        builder.append_keyword(&self.operator_text());
        builder.append_keyword(" ");

        match self {
            Self::IntCompares { value, operator: _ } => {
                builder.append_primitive(&re_format::format_int(*value))
            }
            Self::FloatCompares { value, operator: _ } => {
                builder.append_primitive(&re_format::format_f64(*value))
            }
            Self::StringContains(query) => builder.append_string_value(query),
            Self::BooleanEquals(query) => builder.append_primitive(&format!("{query}")),
        };
    }
}

fn comparison_op_ui(ui: &mut egui::Ui, text: egui::WidgetText, op: &mut ComparisonOperator) {
    egui::ComboBox::new("comp_op", "")
        .selected_text(text)
        .show_ui(ui, |ui| {
            for possible_op in crate::filters::ComparisonOperator::ALL {
                if ui.button(possible_op.to_string()).clicked() {
                    *op = *possible_op;
                }
            }
        });
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

        let mut process_text_edit_response = |ui: &egui::Ui, response: &egui::Response| {
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
        };

        let mut top_text_builder = SyntaxHighlightedBuilder::new();
        top_text_builder.append_body(column_name);
        top_text_builder.append_keyword(" ");
        top_text_builder.append_keyword(&self.operator_text());
        let top_text = top_text_builder.into_widget_text(ui.style());

        match self {
            Self::IntCompares { operator, value } => {
                comparison_op_ui(ui, top_text, operator);

                let mut value_str = value.to_string();
                let response = ui.text_edit_singleline(&mut value_str);
                if response.changed()
                    && let Ok(parsed) = value_str.parse()
                {
                    *value = parsed;
                }

                process_text_edit_response(ui, &response);
            }

            Self::FloatCompares { operator, value } => {
                comparison_op_ui(ui, top_text, operator);

                let mut value_str = value.to_string();
                let response = ui.text_edit_singleline(&mut value_str);
                if response.changed()
                    && let Ok(parsed) = value_str.parse()
                {
                    *value = parsed;
                }

                process_text_edit_response(ui, &response);
            }

            Self::StringContains(query) => {
                ui.label(top_text);
                let response = ui.text_edit_singleline(query);

                process_text_edit_response(ui, &response);
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
    fn operator_text(&self) -> String {
        match self {
            Self::IntCompares { operator, .. } | Self::FloatCompares { operator, .. } => {
                operator.to_string()
            }
            Self::StringContains(_) => "contains".to_owned(),
            Self::BooleanEquals(_) => "is".to_owned(),
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
            use FilterOperation::*;
            let _op = StringContains(String::new());
            match _op {
                IntCompares { .. }
                | FloatCompares { .. }
                | StringContains(_)
                | BooleanEquals(_) => {}
            }
        };

        [
            (
                FilterOperation::IntCompares {
                    operator: ComparisonOperator::Eq,
                    value: 100,
                },
                "int_compare",
            ),
            (
                FilterOperation::FloatCompares {
                    operator: ComparisonOperator::Ge,
                    value: 10.5,
                },
                "float_compares",
            ),
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

    #[test]
    fn test_filter_wrapping() {
        let filters = vec![
            Filter::new(
                "some:column:name",
                FilterOperation::StringContains("some query string".to_owned()),
            ),
            Filter::new(
                "other:column:name",
                FilterOperation::StringContains("hello".to_owned()),
            ),
            Filter::new(
                "short:name",
                FilterOperation::StringContains("world".to_owned()),
            ),
            Filter::new(
                "looooog:name",
                FilterOperation::StringContains("some more querying text here".to_owned()),
            ),
            Filter::new(
                "world",
                FilterOperation::StringContains(":wave:".to_owned()),
            ),
        ];

        let mut filters = FilterState {
            filters,
            active_filter: None,
        };

        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::Vec2::new(700.0, 500.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

                filters.filter_bar_ui(ui, &mut TableBlueprint::default());
            });

        harness.run();

        harness.snapshot("filter_wrapping");
    }
}
