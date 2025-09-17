use std::mem;

use egui::{Atom, AtomLayout, Atoms, Frame, Margin, Sense};

use re_ui::{SyntaxHighlighting, UiExt as _, syntax_highlighting::SyntaxHighlightedBuilder};

use super::{ComparisonOperator, Filter, FilterOperation};
use crate::TableBlueprint;

/// Action to take based on the user interaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FilterUiAction {
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

                ui.horizontal_wrapped(|ui| {
                    for (index, filter) in self.filters.iter_mut().enumerate() {
                        // egui uses this id to store the popup openness and size information,
                        // so we must invalidate if the filter at a given index changes its
                        // nature
                        let filter_id = ui.make_persistent_id(egui::Id::new(index).with(
                            match filter.operation {
                                FilterOperation::IntCompares { .. } => "int",
                                FilterOperation::FloatCompares { .. } => "float",
                                FilterOperation::StringContains(_) => "string",
                                FilterOperation::Boolean(_) => "bool",
                            },
                        ));

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
    pub fn close_button_id() -> egui::Id {
        egui::Id::new("filter_close_button")
    }

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

        let mut atoms = Atoms::default();

        let layout_job = SyntaxHighlightedBuilder::new()
            .with_body_default(&self.column_name)
            .with_keyword(" ")
            .with(&self.operation)
            .into_job(ui.style());

        atoms.push_right(layout_job);

        atoms.push_right(Atom::custom(
            Self::close_button_id(),
            ui.tokens().small_icon_size,
        ));

        let frame = Frame::new()
            .inner_margin(Margin::symmetric(4, 4))
            .stroke(ui.tokens().table_filter_frame_stroke)
            .corner_radius(2.0);

        let atom_layout = AtomLayout::new(atoms).sense(Sense::click()).frame(frame);

        let atom_response = atom_layout.show(ui);

        if let Some(rect) = atom_response.rect(Self::close_button_id()) {
            // The default padding is (1.0, 0.0), making the button look weird
            let button_padding = mem::take(&mut ui.style_mut().spacing.button_padding);
            if ui
                .place(
                    rect,
                    ui.small_icon_button_widget(&re_ui::icons::CLOSE_SMALL, "Remove filter")
                        // Without small the button would grow to interact_size and be off-center
                        .small(),
                )
                .clicked()
            {
                should_delete_filter = true;
                action_due_to_filter_deletion = FilterUiAction::CommitStateToBlueprint;
            }
            ui.style_mut().spacing.button_padding = button_padding;
        }

        let response = atom_response.response;

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
                if let Some(value) = value {
                    builder.append_primitive(&re_format::format_int(*value));
                } else {
                    builder.append_primitive("…");
                }
            }

            Self::FloatCompares { value, operator: _ } => {
                if let Some(value) = value {
                    builder.append_primitive(&re_format::format_f64(value.into_inner()));
                } else {
                    builder.append_primitive("…");
                }
            }

            Self::StringContains(query) => {
                builder.append_string_value(query);
            }

            Self::Boolean(boolean_filter) => {
                builder.append_primitive(&boolean_filter.operand_text());
            }
        }
    }
}

fn numerical_comparison_operator_ui(
    ui: &mut egui::Ui,
    column_name: &str,
    operator_text: &str,
    op: &mut ComparisonOperator,
) {
    ui.horizontal(|ui| {
        ui.label(SyntaxHighlightedBuilder::body_default(column_name).into_widget_text(ui.style()));

        egui::ComboBox::new("comp_op", "")
            .selected_text(
                SyntaxHighlightedBuilder::keyword(operator_text).into_widget_text(ui.style()),
            )
            .show_ui(ui, |ui| {
                for possible_op in crate::filters::ComparisonOperator::ALL {
                    if ui
                        .button(
                            SyntaxHighlightedBuilder::keyword(&possible_op.to_string())
                                .into_widget_text(ui.style()),
                        )
                        .clicked()
                    {
                        *op = *possible_op;
                    }
                }
            });
    });
}

pub fn basic_operation_ui(ui: &mut egui::Ui, column_name: &str, operator_text: &str) {
    ui.label(
        SyntaxHighlightedBuilder::body_default(column_name)
            .with_keyword(" ")
            .with_keyword(operator_text)
            .into_widget_text(ui.style()),
    );
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

        let operator_text = self.operator_text();

        // Reduce the default width unnecessarily expands the popup width (queries as usually vers
        // small).
        ui.spacing_mut().text_edit_width = 150.0;

        // TODO(ab): this is getting unwieldy. All arms should have an independent inner struct,
        // which all handle their own UI.
        match self {
            Self::IntCompares { operator, value } => {
                numerical_comparison_operator_ui(ui, column_name, &operator_text, operator);

                let mut value_str = value.map(|v| v.to_string()).unwrap_or_default();
                let response = ui.text_edit_singleline(&mut value_str);
                if response.changed() {
                    if value_str.is_empty() {
                        *value = None;
                    } else if let Ok(parsed) = value_str.parse() {
                        *value = Some(parsed);
                    }
                }

                process_text_edit_response(ui, &response);
            }

            Self::FloatCompares { operator, value } => {
                numerical_comparison_operator_ui(ui, column_name, &operator_text, operator);

                let mut value_str = value.map(|v| v.to_string()).unwrap_or_default();
                let response = ui.text_edit_singleline(&mut value_str);
                if response.changed() {
                    if value_str.is_empty() {
                        *value = None;
                    } else if let Ok(parsed) = value_str.parse() {
                        *value = Some(parsed);
                    }
                }

                process_text_edit_response(ui, &response);
            }

            Self::StringContains(query) => {
                basic_operation_ui(ui, column_name, &operator_text);

                let response = ui.text_edit_singleline(query);

                process_text_edit_response(ui, &response);
            }

            Self::Boolean(boolean_filter) => {
                boolean_filter.popup_ui(ui, column_name, &mut action);
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
            Self::Boolean(_) => "is".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use ordered_float::OrderedFloat;

    use super::*;
    use crate::filters::BooleanFilter;

    fn test_cases() -> Vec<(FilterOperation, &'static str)> {
        // Let's remember to update this test when adding new filter operations.
        #[cfg(debug_assertions)]
        let _: () = {
            use FilterOperation::*;
            let _op = StringContains(String::new());
            match _op {
                IntCompares { .. } | FloatCompares { .. } | StringContains(_) | Boolean(_) => {}
            }
        };

        [
            (
                FilterOperation::IntCompares {
                    operator: ComparisonOperator::Eq,
                    value: Some(100),
                },
                "int_compare",
            ),
            (
                FilterOperation::IntCompares {
                    operator: ComparisonOperator::Eq,
                    value: None,
                },
                "int_compare_none",
            ),
            (
                FilterOperation::FloatCompares {
                    operator: ComparisonOperator::Ge,
                    value: Some(OrderedFloat(10.5)),
                },
                "float_compares",
            ),
            (
                FilterOperation::FloatCompares {
                    operator: ComparisonOperator::Ge,
                    value: None,
                },
                "float_compares_none",
            ),
            (
                FilterOperation::StringContains("query".to_owned()),
                "string_contains",
            ),
            (
                FilterOperation::StringContains(String::new()),
                "string_contains_empty",
            ),
            (
                FilterOperation::Boolean(BooleanFilter::NonNullable(true)),
                "boolean_equals_true",
            ),
            (
                FilterOperation::Boolean(BooleanFilter::NonNullable(false)),
                "boolean_equals_false",
            ),
            (
                FilterOperation::Boolean(BooleanFilter::Nullable(Some(true))),
                "nullable_boolean_equals_true",
            ),
            (
                FilterOperation::Boolean(BooleanFilter::Nullable(Some(false))),
                "nullable_boolean_equals_false",
            ),
            (
                FilterOperation::Boolean(BooleanFilter::Nullable(None)),
                "nullable_boolean_equals_null",
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
