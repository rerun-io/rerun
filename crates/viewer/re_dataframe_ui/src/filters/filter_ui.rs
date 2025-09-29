use std::mem;

use egui::{Atom, AtomLayout, Atoms, Frame, Margin, Sense};

use re_log_types::TimestampFormat;
use re_ui::{SyntaxHighlighting, UiExt as _, syntax_highlighting::SyntaxHighlightedBuilder};

use super::{Filter, FilterKind, TimestampFormatted};
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
    pub fn merge(self, other: Self) -> Self {
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
    pub fn filter_bar_ui(
        &mut self,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
        table_blueprint: &mut TableBlueprint,
    ) {
        // From there on, we always want to show the "today" date, because not doing so leads
        // to some very confusing display.
        let timestamp_format = timestamp_format.with_hide_today_date(false);

        let action = self.filter_bar_ui_impl(ui, timestamp_format);

        match action {
            FilterUiAction::None => {}

            FilterUiAction::CommitStateToBlueprint => {
                // give a chance to filters to clean themselves up before committing to the table
                // blueprint
                for filter in &mut self.filters {
                    filter.kind.on_commit();
                }
                table_blueprint.filters = self.filters.clone();
            }

            FilterUiAction::CancelStateEdit => {
                self.filters = table_blueprint.filters.clone();
                self.active_filter = None;
            }
        }
    }

    #[must_use]
    fn filter_bar_ui_impl(
        &mut self,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
    ) -> FilterUiAction {
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
                        // name.
                        let filter_id =
                            ui.make_persistent_id(egui::Id::new(index).with(&filter.column_name));

                        let result =
                            filter.ui(ui, timestamp_format, filter_id, Some(index) == active_index);

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
        timestamp_format: TimestampFormat,
        filter_id: egui::Id,
        activate_filter: bool,
    ) -> DisplayFilterUiResult {
        let mut should_delete_filter = false;
        let mut action_due_to_filter_deletion = FilterUiAction::None;

        let mut atoms = Atoms::default();

        let layout_job = SyntaxHighlightedBuilder::new()
            .with_body_default(&self.column_name)
            .with_keyword(" ")
            .with(&TimestampFormatted::new(&self.kind, timestamp_format))
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
            // The default text edit background is too dark for the (lighter) background of popups,
            // so we switch to a lighter shade.
            ui.visuals_mut().text_edit_bg_color = Some(ui.visuals().widgets.inactive.bg_fill);

            let action = self.kind.popup_ui(
                ui,
                timestamp_format,
                self.column_name.as_ref(),
                popup_was_closed,
            );

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

impl SyntaxHighlighting for TimestampFormatted<'_, FilterKind> {
    fn syntax_highlight_into(&self, builder: &mut SyntaxHighlightedBuilder) {
        //TODO(ab): this is weird. this entire impl should be delegated to inner structs
        builder.append_keyword(&self.inner.operator_text());
        builder.append_keyword(" ");

        match self.inner {
            FilterKind::NonNullableBoolean(boolean_filter) => {
                builder.append_primitive(&boolean_filter.operand_text());
            }

            FilterKind::NullableBoolean(boolean_filter) => {
                builder.append_primitive(&boolean_filter.operand_text());
            }

            FilterKind::Int(int_filter) => {
                builder.append(int_filter);
            }

            FilterKind::Float(float_filter) => {
                builder.append(float_filter);
            }

            FilterKind::String(string_filter) => {
                builder.append(string_filter);
            }

            FilterKind::Timestamp(timestamp_filter) => {
                builder.append(&self.convert(timestamp_filter));
            }
        }
    }
}

pub fn basic_operation_ui(ui: &mut egui::Ui, column_name: &str, operator_text: &str) {
    ui.label(
        SyntaxHighlightedBuilder::body_default(column_name)
            .with_keyword(" ")
            .with_keyword(operator_text)
            .into_widget_text(ui.style()),
    );
}

impl FilterKind {
    /// Returns true if the filter must be committed.
    fn popup_ui(
        &mut self,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
        column_name: &str,
        popup_just_opened: bool,
    ) -> FilterUiAction {
        // Reduce the default width unnecessarily expands the popup width (queries as usually vers
        // small).
        ui.spacing_mut().text_edit_width = 150.0;

        match self {
            Self::NonNullableBoolean(boolean_filter) => boolean_filter.popup_ui(ui, column_name),
            Self::NullableBoolean(boolean_filter) => boolean_filter.popup_ui(ui, column_name),
            Self::Int(int_filter) => int_filter.popup_ui(ui, column_name, popup_just_opened),
            Self::Float(float_filter) => float_filter.popup_ui(ui, column_name, popup_just_opened),
            Self::String(string_filter) => {
                string_filter.popup_ui(ui, column_name, popup_just_opened)
            }
            Self::Timestamp(timestamp_filter) => {
                timestamp_filter.popup_ui(ui, column_name, timestamp_format)
            }
        }
    }

    /// Given a chance to the underlying filter struct to update/clean itself upon committing the
    /// filter state to the table blueprint.
    ///
    /// This is used e.g. by the timestamp filter to normalize the user entry to the proper
    /// representation of the parsed timestamp.
    fn on_commit(&mut self) {
        match self {
            Self::NullableBoolean(_)
            | Self::NonNullableBoolean(_)
            | Self::Int(_)
            | Self::Float(_)
            | Self::String(_) => {}

            Self::Timestamp(timestamp_filter) => timestamp_filter.on_commit(),
        }
    }

    /// Display text of the operator.
    fn operator_text(&self) -> String {
        match self {
            Self::Int(int_filter) => int_filter.comparison_operator().to_string(),
            Self::Float(float_filter) => float_filter.comparison_operator().to_string(),
            Self::String(string_filter) => string_filter.operator().to_string(),
            Self::NonNullableBoolean(_) | Self::NullableBoolean(_) | Self::Timestamp(_) => {
                "is".to_owned()
            }
        }
    }
}

/// Get a filter ui action from a text edit response.
pub fn action_from_text_edit_response(ui: &egui::Ui, response: &egui::Response) -> FilterUiAction {
    if response.lost_focus() {
        ui.input(|i| {
            if i.key_pressed(egui::Key::Enter) {
                FilterUiAction::CommitStateToBlueprint
            } else if i.key_pressed(egui::Key::Escape) {
                FilterUiAction::CancelStateEdit
            } else {
                FilterUiAction::None
            }
        })
    } else {
        FilterUiAction::None
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        ComparisonOperator, FloatFilter, IntFilter, NonNullableBooleanFilter,
        NullableBooleanFilter, StringFilter, TimestampFilter,
    };
    use super::*;
    use crate::filters::StringOperator;

    fn test_cases() -> Vec<(FilterKind, &'static str)> {
        // Let's remember to update this test when adding new filter operations.
        #[cfg(debug_assertions)]
        let _: () = {
            use FilterKind::*;
            let _op = String(Default::default());
            match _op {
                NonNullableBoolean(_)
                | NullableBoolean(_)
                | Int(_)
                | Float(_)
                | String(_)
                | Timestamp(_) => {}
            }
        };

        [
            (
                FilterKind::NonNullableBoolean(NonNullableBooleanFilter::IsTrue),
                "boolean_equals_true",
            ),
            (
                FilterKind::NonNullableBoolean(NonNullableBooleanFilter::IsFalse),
                "boolean_equals_false",
            ),
            (
                FilterKind::NullableBoolean(NullableBooleanFilter::IsTrue),
                "nullable_boolean_equals_true",
            ),
            (
                FilterKind::NullableBoolean(NullableBooleanFilter::IsFalse),
                "nullable_boolean_equals_false",
            ),
            (
                FilterKind::NullableBoolean(NullableBooleanFilter::IsNull),
                "nullable_boolean_equals_null",
            ),
            (
                FilterKind::Int(IntFilter::new(ComparisonOperator::Eq, Some(100))),
                "int_compare",
            ),
            (
                FilterKind::Int(IntFilter::new(ComparisonOperator::Eq, None)),
                "int_compare_none",
            ),
            (
                FilterKind::Float(FloatFilter::new(ComparisonOperator::Ge, Some(10.5))),
                "float_compares",
            ),
            (
                FilterKind::Float(FloatFilter::new(ComparisonOperator::Ge, None)),
                "float_compares_none",
            ),
            (
                FilterKind::String(StringFilter::new(StringOperator::Contains, "query")),
                "string_contains",
            ),
            (
                FilterKind::String(StringFilter::new(StringOperator::Contains, "")),
                "string_contains_empty",
            ),
            (
                FilterKind::String(StringFilter::new(StringOperator::StartsWith, "query")),
                "string_starts_with",
            ),
            (
                FilterKind::Timestamp(TimestampFilter::after(
                    jiff::Timestamp::from_millisecond(100_000_000_000).unwrap(),
                )),
                "timestamp_after",
            ),
            (
                FilterKind::Timestamp(TimestampFilter::between(
                    jiff::Timestamp::from_millisecond(100_000_000_000).unwrap(),
                    jiff::Timestamp::from_millisecond(110_000_000_000).unwrap(),
                )),
                "timestamp_between",
            ),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn test_filter_ui() {
        for (filter_op, test_name) in test_cases() {
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::Vec2::new(800.0, 80.0))
                .build_ui(|ui| {
                    re_ui::apply_style_and_install_loaders(ui.ctx());

                    let mut filter_state = FilterState {
                        filters: vec![Filter::new("column:name".to_owned(), filter_op.clone())],
                        active_filter: None,
                    };

                    let _res = filter_state.filter_bar_ui_impl(ui, TimestampFormat::utc());
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

                    egui::Popup::new(
                        ui.id().with("popup"),
                        ui.ctx().clone(),
                        egui::Rect::from_min_size(
                            egui::pos2(0., 0.),
                            egui::vec2(ui.available_width(), 0.0),
                        ),
                        ui.layer_id(),
                    )
                    .open(true)
                    .show(|ui| {
                        ui.visuals_mut().text_edit_bg_color =
                            Some(ui.visuals().widgets.inactive.bg_fill);

                        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

                        let _res =
                            filter_op.popup_ui(ui, TimestampFormat::utc(), "column:name", true);
                    });
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
                FilterKind::String(StringFilter::new(
                    StringOperator::Contains,
                    "some query string".to_owned(),
                )),
            ),
            Filter::new(
                "other:column:name",
                FilterKind::String(StringFilter::new(
                    StringOperator::Contains,
                    "hello".to_owned(),
                )),
            ),
            Filter::new(
                "short:name",
                FilterKind::String(StringFilter::new(
                    StringOperator::Contains,
                    "world".to_owned(),
                )),
            ),
            Filter::new(
                "looooog:name",
                FilterKind::String(StringFilter::new(
                    StringOperator::Contains,
                    "some more querying text here".to_owned(),
                )),
            ),
            Filter::new(
                "world",
                FilterKind::String(StringFilter::new(
                    StringOperator::Contains,
                    ":wave:".to_owned(),
                )),
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

                filters.filter_bar_ui(ui, TimestampFormat::utc(), &mut TableBlueprint::default());
            });

        harness.run();

        harness.snapshot("filter_wrapping");
    }
}
