use std::mem;

use egui::{Atom, AtomLayout, Atoms, Frame, Margin, Sense};
use re_log_types::TimestampFormat;
use re_ui::UiExt as _;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;

use super::{ColumnFilter, Filter as _, TimestampFormatted};
use crate::TableBlueprint;

/// Action to take based on the user interaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FilterUiAction {
    #[default]
    None,

    /// The user closed the filter popup using enter or by clicking outside. The updated filter
    /// state should be committed to the table blueprint.
    CommitStateToBlueprint,

    /// The user closed the filter popup using escape, so the edit should be canceled by resetting
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
/// differ from the content of [`TableBlueprint::column_filters`]. [`Self::filter_bar_ui`] returns a
/// flag to indicate when this content should be committed to the blueprint.
#[derive(Clone, Debug)]
pub struct FilterState {
    pub column_filters: Vec<ColumnFilter>,
    pub active_filter: Option<usize>,
}

impl FilterState {
    /// Restore the saved state, initializing it from the blueprint if needed.
    ///
    /// Call this at the beginning of the frame.
    pub fn load_or_init_from_blueprint(
        egui_ctx: &egui::Context,
        persisted_id: egui::Id,
        table_blueprint: &TableBlueprint,
    ) -> Self {
        egui_ctx.data_mut(|data| {
            data.get_temp_mut_or_insert_with(persisted_id, || Self {
                column_filters: table_blueprint.column_filters.clone(),
                active_filter: None,
            })
            .clone()
        })
    }

    /// Store the state to the temporary memory.
    ///
    /// Call this at the end of the frame.
    pub fn store(self, egui_ctx: &egui::Context, persisted_id: egui::Id) {
        egui_ctx.data_mut(|data| {
            data.insert_temp(persisted_id, self);
        });
    }

    /// Add a new filter to the filter bar.
    pub fn push_new_filter(&mut self, filter: ColumnFilter) {
        self.column_filters.push(filter);
        self.active_filter = Some(self.column_filters.len() - 1);
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
                for column_filter in &mut self.column_filters {
                    column_filter.filter.on_commit();
                }
                table_blueprint.column_filters = self.column_filters.clone();
            }

            FilterUiAction::CancelStateEdit => {
                self.column_filters = table_blueprint.column_filters.clone();
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
        if self.column_filters.is_empty() {
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
                    for (index, column_filter) in self.column_filters.iter_mut().enumerate() {
                        // egui uses this id to store the popup openness and size information,
                        // so we must invalidate if the filter at a given index changes its
                        // name.
                        let filter_id = ui.make_persistent_id(
                            egui::Id::new(index).with(column_filter.field.name()),
                        );

                        let result = column_filter.ui(
                            ui,
                            timestamp_format,
                            filter_id,
                            Some(index) == active_index,
                        );

                        action = action.merge(result.filter_action);

                        if result.should_delete_filter {
                            remove_idx = Some(index);
                        }
                    }

                    if let Some(remove_idx) = remove_idx {
                        self.active_filter = None;
                        self.column_filters.remove(remove_idx);
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

impl ColumnFilter {
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
            .with_body_default(self.field.name())
            .with_keyword(" ")
            .with(&TimestampFormatted::new(&self.filter, timestamp_format))
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

            let action =
                self.filter
                    .popup_ui(ui, timestamp_format, self.field.name(), popup_was_closed);

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
    use std::sync::Arc;

    use arrow::datatypes::{DataType, Field, FieldRef};
    use egui::accesskit::Role;
    use egui::{Key, Modifiers};
    use egui_kittest::SnapshotResults;
    use egui_kittest::kittest::Queryable as _;

    use super::super::{
        ComparisonOperator, FloatFilter, IntFilter, NonNullableBooleanFilter,
        NullableBooleanFilter, StringFilter, StringOperator, TimestampFilter, TypedFilter,
    };
    use super::*;

    fn test_cases() -> Vec<(TypedFilter, &'static str)> {
        // Let's remember to update this test when adding new filter types.
        #[cfg(debug_assertions)]
        let _: () = {
            use TypedFilter::*;
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
                NonNullableBooleanFilter::IsTrue.into(),
                "boolean_equals_true",
            ),
            (
                NonNullableBooleanFilter::IsFalse.into(),
                "boolean_equals_false",
            ),
            (
                NullableBooleanFilter::new_is_true().into(),
                "nullable_boolean_equals_true",
            ),
            (
                NullableBooleanFilter::new_is_true().with_is_not().into(),
                "nullable_boolean_not_equals_true",
            ),
            (
                NullableBooleanFilter::new_is_false().into(),
                "nullable_boolean_equals_false",
            ),
            (
                NullableBooleanFilter::new_is_null().into(),
                "nullable_boolean_equals_null",
            ),
            (
                IntFilter::new(ComparisonOperator::Eq, Some(100)).into(),
                "int_compare",
            ),
            (
                IntFilter::new(ComparisonOperator::Eq, None).into(),
                "int_compare_none",
            ),
            (
                FloatFilter::new(ComparisonOperator::Ge, Some(10.5)).into(),
                "float_compares",
            ),
            (
                FloatFilter::new(ComparisonOperator::Ge, None).into(),
                "float_compares_none",
            ),
            (
                StringFilter::new(StringOperator::Contains, "query").into(),
                "string_contains",
            ),
            (
                StringFilter::new(StringOperator::Contains, "").into(),
                "string_contains_empty",
            ),
            (
                StringFilter::new(StringOperator::StartsWith, "query").into(),
                "string_starts_with",
            ),
            (
                TimestampFilter::after(jiff::Timestamp::from_millisecond(100_000_000_000).unwrap())
                    .into(),
                "timestamp_after",
            ),
            (
                TimestampFilter::after(jiff::Timestamp::from_millisecond(100_000_000_000).unwrap())
                    .with_is_not()
                    .into(),
                "timestamp_not_after",
            ),
            (
                TimestampFilter::between(
                    jiff::Timestamp::from_millisecond(100_000_000_000).unwrap(),
                    jiff::Timestamp::from_millisecond(110_000_000_000).unwrap(),
                )
                .into(),
                "timestamp_between",
            ),
        ]
        .into_iter()
        .collect()
    }

    fn dummy_field(name: &str) -> FieldRef {
        // the actual data type is irrelevant for these tests
        Arc::new(Field::new(name, DataType::Int64, false))
    }

    #[test]
    fn test_filter_ui() {
        let mut snapshot_results = SnapshotResults::new();
        for (filter, test_name) in test_cases() {
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::Vec2::new(800.0, 80.0))
                .build_ui(|ui| {
                    re_ui::apply_style_and_install_loaders(ui.ctx());

                    let mut filter_state = FilterState {
                        column_filters: vec![ColumnFilter::new(
                            dummy_field("column:name"),
                            filter.clone(),
                        )],
                        active_filter: None,
                    };

                    let _res = filter_state.filter_bar_ui_impl(ui, TimestampFormat::utc());
                });

            harness.run();

            harness.snapshot(format!("filter_ui_{test_name}"));

            snapshot_results.extend_harness(&mut harness);
        }
    }

    #[test]
    fn test_popup_ui() {
        let mut snapshot_results = SnapshotResults::new();
        for (mut filter_op, test_name) in test_cases() {
            let mut harness = egui_kittest::Harness::builder()
                .with_size(egui::Vec2::new(400.0, 400.0))
                .build_ui(|ui| {
                    re_ui::apply_style_and_install_loaders(ui.ctx());

                    egui::Popup::new(
                        ui.id().with("popup"),
                        ui.ctx().clone(),
                        egui::Rect::from_min_size(
                            egui::pos2(10., 10.),
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

            snapshot_results.extend_harness(&mut harness);
        }
    }

    #[test]
    fn test_filter_wrapping() {
        let filters = vec![
            ColumnFilter::new(
                dummy_field("some:column:name"),
                StringFilter::new(StringOperator::Contains, "some query string".to_owned()),
            ),
            ColumnFilter::new(
                dummy_field("other:column:name"),
                StringFilter::new(StringOperator::Contains, "hello".to_owned()),
            ),
            ColumnFilter::new(
                dummy_field("short:name"),
                StringFilter::new(StringOperator::Contains, "world".to_owned()),
            ),
            ColumnFilter::new(
                dummy_field("looooog:name"),
                StringFilter::new(
                    StringOperator::Contains,
                    "some more querying text here".to_owned(),
                ),
            ),
            ColumnFilter::new(
                dummy_field("world"),
                StringFilter::new(StringOperator::Contains, ":wave:".to_owned()),
            ),
        ];

        let mut filters = FilterState {
            column_filters: filters,
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

    /// This test runs through a full edit cycle of a timestamp filter, and assess that the
    /// timestamp string is normalized after commit—that is, the timestamp string is set to the
    /// canonical representation of the previously entered timestamp.
    #[test]
    fn test_timestamp_filter_on_commit() {
        let filters = vec![ColumnFilter::new(
            dummy_field("some:column:name"),
            TimestampFilter::after(jiff::Timestamp::from_millisecond(100_000_000_000).unwrap()),
        )];

        let mut filters = FilterState {
            column_filters: filters,
            active_filter: None,
        };

        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::Vec2::new(400.0, 400.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

                filters.filter_bar_ui(ui, TimestampFormat::utc(), &mut TableBlueprint::default());
            });

        // Open the popup for the timeststamp filter.
        harness.run();
        let node = harness.get_by_role(Role::Unknown);
        node.click();
        harness.run();

        // Activate the text input and select all.
        harness.key_press(Key::Tab);
        harness.run();
        let text_input = harness.get_by_role(Role::TextInput);
        text_input.click();
        harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
        harness.run();

        // Enter a timestamp string and commit the change.
        let text_input = harness.get_by_role(Role::TextInput);
        text_input.type_text("1979-07-10");
        harness.key_press(Key::Enter);
        harness.run();

        // Open the popup again.
        let node = harness.get_by_role(Role::Unknown);
        node.click();
        harness.run();

        harness.snapshot("timestamp_filter_on_commit");
    }
}
