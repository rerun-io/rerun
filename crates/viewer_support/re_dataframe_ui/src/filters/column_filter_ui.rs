use std::mem;

use arrow::datatypes::Schema;
use egui::{Atom, Atoms, Frame, Margin, Sense, WidgetAtom};
use re_log_types::TimestampFormat;
use re_ui::UiExt as _;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;

use super::{ColumnFilter, CustomFilter, Filter as _, TableFilter, TimestampFormatted};
use crate::datafusion_adapter::FilterErrors;

/// Action to take based on the user interaction.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FilterUiAction {
    #[default]
    None,

    /// The user closed the filter popup using enter or by clicking outside. The updated filter
    /// state should be committed to the table blueprint.
    CommitStateToBlueprint,

    /// The user closed the filter popup using escape, so the edit is discarded and the filter bar
    /// shows the table blueprint again.
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
/// While the user edits a filter, the state is stored in egui temp memory and can differ from the
/// filters in the table blueprint. Otherwise [`Self::load`] reads the filters from the blueprint.
/// [`Self::filter_bar_ui`] returns the filters to write to the blueprint when the user commits an
/// edit.
#[derive(Clone, Debug, Default)]
pub struct FilterState {
    filters: Vec<TableFilter>,
    active_filter: Option<usize>,

    /// Whether a filter popup was open in the last [`Self::filter_bar_ui`].
    popup_open: bool,
}

impl FilterState {
    pub fn new(filters: Vec<TableFilter>) -> Self {
        Self {
            filters,
            active_filter: None,
            popup_open: false,
        }
    }

    /// Restore the state of an ongoing edit, or read the filters of the table blueprint.
    ///
    /// Call this at the beginning of the frame.
    pub fn load(
        egui_ctx: &egui::Context,
        persisted_id: egui::Id,
        blueprint_filters: &[String],
        schema: &Schema,
    ) -> Self {
        egui_ctx
            .data(|data| data.get_temp(persisted_id))
            .unwrap_or_else(|| {
                Self::new(
                    blueprint_filters
                        .iter()
                        .map(|sql| TableFilter::from_sql(sql, schema))
                        .collect(),
                )
            })
    }

    /// Store the state to the temporary memory while the user edits a filter.
    ///
    /// Call this at the end of the frame.
    pub fn store(self, egui_ctx: &egui::Context, persisted_id: egui::Id) {
        egui_ctx.data_mut(|data| {
            if self.popup_open || self.active_filter.is_some() {
                data.insert_temp(persisted_id, self);
            } else {
                data.remove_temp::<Self>(persisted_id);
            }
        });
    }

    /// Add a new filter to the filter bar and open its popup.
    pub fn push_new_filter(&mut self, filter: ColumnFilter) {
        self.filters.push(TableFilter::Column(filter));
        self.active_filter = Some(self.filters.len() - 1);
    }

    /// Display the filter bar UI.
    ///
    /// A custom filter with an entry in `filter_errors` shows as an error pill.
    ///
    /// Returns the filters to write to the table blueprint when the user commits an edit.
    /// Filters that select every row are left out.
    #[must_use]
    pub fn filter_bar_ui(
        &mut self,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
        filter_errors: &FilterErrors,
    ) -> Option<Vec<String>> {
        // From there on, we always want to show the "today" date, because not doing so leads
        // to some very confusing display.
        let timestamp_format =
            timestamp_format.with_date_visibility(re_log_types::DateVisibility::ShowDate);

        let action = self.filter_bar_ui_impl(ui, timestamp_format, filter_errors);

        // On cancel the state isn't stored, so the next frame reads the blueprint again.
        if action != FilterUiAction::CommitStateToBlueprint {
            return None;
        }

        // give a chance to filters to clean themselves up before committing to the table
        // blueprint
        for filter in &mut self.filters {
            if let TableFilter::Column(column_filter) = filter {
                column_filter.filter.on_commit();
            }
        }
        Some(
            self.filters
                .iter()
                .filter_map(TableFilter::to_sql)
                .collect(),
        )
    }

    #[must_use]
    fn filter_bar_ui_impl(
        &mut self,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
        filter_errors: &FilterErrors,
    ) -> FilterUiAction {
        self.popup_open = false;

        if self.filters.is_empty() {
            return Default::default();
        }

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
                        let result = match filter {
                            TableFilter::Column(column_filter) => {
                                // egui uses this id to store the popup openness and size
                                // information, so we must invalidate if the filter at a given
                                // index changes its name.
                                let filter_id =
                                    ui.make_persistent_id((index, column_filter.field.name()));

                                column_filter.ui(
                                    ui,
                                    timestamp_format,
                                    filter_id,
                                    Some(index) == active_index,
                                )
                            }
                            TableFilter::Custom(custom_filter) => custom_filter_ui(
                                ui,
                                custom_filter,
                                filter_errors.get(&custom_filter.sql).map(String::as_str),
                            ),
                        };

                        action = action.merge(result.filter_action);
                        self.popup_open |= result.popup_open;

                        if result.should_delete_filter {
                            remove_idx = Some(index);
                        }
                    }

                    if let Some(remove_idx) = remove_idx {
                        self.filters.remove(remove_idx);
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
    popup_open: bool,
}

impl ColumnFilter {
    pub fn close_button_id() -> egui::IdSalt {
        egui::IdSalt::new("filter_close_button")
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
        let layout_job = SyntaxHighlightedBuilder::new()
            .with_body_default(self.field.name())
            .with_keyword(" ")
            .with(&TimestampFormatted::new(&self.filter, timestamp_format))
            .to_job(ui.style());

        let (response, should_delete_filter) = filter_pill_ui(ui, None, layout_job);
        let action_due_to_filter_deletion = if should_delete_filter {
            FilterUiAction::CommitStateToBlueprint
        } else {
            FilterUiAction::None
        };

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

        DisplayFilterUiResult {
            filter_action: action_due_to_filter_deletion
                .merge(action_due_to_closed_popup)
                .merge(action_from_popup_ui),
            should_delete_filter,
            popup_open,
        }
    }
}

/// A filter pill with an optional leading icon and a button to remove the filter.
///
/// Returns the pill's response, and whether the remove button was clicked.
fn filter_pill_ui(
    ui: &mut egui::Ui,
    icon: Option<egui::Image<'static>>,
    layout_job: egui::text::LayoutJob,
) -> (egui::Response, bool) {
    let label = layout_job.text.clone();
    let mut atoms = Atoms::default();
    if let Some(icon) = icon {
        atoms.push_right(icon.fit_to_exact_size(ui.tokens().small_icon_size));
    }
    atoms.push_right(layout_job);
    atoms.push_right(Atom::custom(
        ColumnFilter::close_button_id(),
        ui.tokens().small_icon_size,
    ));

    let frame = Frame::new()
        .inner_margin(Margin::symmetric(4, 4))
        .stroke(ui.tokens().table_filter_frame_stroke)
        .corner_radius(2.0);

    let atom_layout = WidgetAtom::new(atoms).sense(Sense::click()).frame(frame);

    let atom_response = atom_layout.show(ui);

    let mut should_delete_filter = false;
    if let Some(rect) = atom_response.rect(ColumnFilter::close_button_id()) {
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
        }
        ui.style_mut().spacing.button_padding = button_padding;
    }

    (
        atom_response.response.accessible_name(label),
        should_delete_filter,
    )
}

/// UI for a filter the filter bar can't edit. Shows its SQL and error (if any) on hover.
///
/// The pill names the leftmost column of the SQL, as in `Custom "some:column" filter`. A failing
/// filter shows in the error color with an error icon.
fn custom_filter_ui(
    ui: &mut egui::Ui,
    custom_filter: &CustomFilter,
    error: Option<&str>,
) -> DisplayFilterUiResult {
    #[cfg(feature = "analytics")]
    {
        let has_shown_custom_filter_id = egui::Id::unique("has_shown_custom_table_filter");
        let has_shown_custom_filter = ui.data_mut(|data| {
            mem::replace(
                data.get_temp_mut_or_default::<bool>(has_shown_custom_filter_id),
                true,
            )
        });
        if !has_shown_custom_filter {
            re_analytics::record(|| re_analytics::event::CustomTableFilterShown {});
        }
    }

    let layout_job = match (&custom_filter.column, error) {
        (None, None) => SyntaxHighlightedBuilder::new().with_keyword("Custom"),
        (Some(column), None) => SyntaxHighlightedBuilder::new()
            .with_keyword("Custom ")
            .with_body_default(&format!(r#""{column}""#))
            .with_keyword(" filter"),
        (column, Some(_)) => {
            let text = match column {
                Some(column) => format!(r#"Custom "{column}" filter"#),
                None => "Error".to_owned(),
            };
            SyntaxHighlightedBuilder::new().with_format(
                &text,
                egui::TextFormat::simple(
                    egui::TextStyle::Body.resolve(ui.style()),
                    ui.visuals().error_fg_color,
                ),
            )
        }
    }
    .to_job(ui.style());

    let icon = error.map(|_| {
        re_ui::icons::ERROR
            .as_image()
            .tint(ui.tokens().alert_error.icon)
    });

    let (response, should_delete_filter) = filter_pill_ui(ui, icon, layout_job);
    response.on_hover_ui(|ui| {
        ui.label(egui::RichText::new(&custom_filter.sql).monospace());
        if let Some(error) = error {
            ui.error_label(error);
        }
    });

    DisplayFilterUiResult {
        filter_action: if should_delete_filter {
            FilterUiAction::CommitStateToBlueprint
        } else {
            FilterUiAction::None
        },
        should_delete_filter,
        popup_open: false,
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
    use egui_kittest::SnapshotResults;

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

                    let mut filter_state = FilterState::new(vec![TableFilter::Column(
                        ColumnFilter::new(dummy_field("column:name"), filter.clone()),
                    )]);

                    let _res = filter_state.filter_bar_ui_impl(
                        ui,
                        TimestampFormat::utc(),
                        &Default::default(),
                    );
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
                        ui.make_persistent_id("popup"),
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

        let mut filters = FilterState::new(filters.into_iter().map(TableFilter::Column).collect());

        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::Vec2::new(700.0, 500.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());

                let _filters_to_save =
                    filters.filter_bar_ui(ui, TimestampFormat::utc(), &Default::default());
            });

        harness.run();

        harness.snapshot("filter_wrapping");
    }
}
