use arrow::datatypes::Schema;
use egui::{Atom, Atoms, Frame, Margin, Sense, WidgetAtom};
use re_log_types::TimestampFormat;
use re_sdk_types::blueprint::components::ColumnName;
use re_ui::filter_widget::{FilterMatcher, PathRanges, format_matching_text};
use re_ui::list_item::CustomContent;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_ui::text_edit::{ReTextEdit, TextEditVariant};
use re_ui::{ButtonExt as _, Size, UiExt as _, icons};

use super::{ColumnFilter, CustomFilter, Filter as _, TableFilter, TimestampFormatted};
use crate::datafusion_adapter::FilterErrors;

/// Width of the popup listing the columns one can filter on.
const COLUMN_PICKER_WIDTH: f32 = 232.0;

/// Past this height, the column picker scrolls.
const COLUMN_PICKER_MAX_HEIGHT: f32 = 360.0;

pub struct FilterableColumn {
    pub display_name: String,
    pub column_filter: ColumnFilter,
}

pub struct FilterControls {
    pub columns: Vec<FilterableColumn>,
    pub hidden_columns: Vec<FilterableColumn>,
}

/// The data type of a column, as shown at the trailing edge of the column picker.
fn datatype_badge(ui: &egui::Ui, filter: &ColumnFilter) -> egui::WidgetText {
    fn type_name(datatype: &arrow::datatypes::DataType) -> String {
        use arrow::datatypes::DataType;

        match datatype {
            DataType::List(field)
            | DataType::LargeList(field)
            | DataType::ListView(field)
            | DataType::LargeListView(field)
            | DataType::FixedSizeList(field, _) => {
                format!("List<{}>", type_name(field.data_type()))
            }

            // The time unit and time zone are noise here: we always display timestamps using the
            // viewer's own timestamp format.
            DataType::Timestamp(_, _) => "Timestamp".to_owned(),

            _ => datatype.to_string(),
        }
    }

    let datatype = type_name(filter.field.data_type());

    let color = if matches!(filter.filter, super::TypedFilter::String(_)) {
        ui.tokens().code_string_color
    } else {
        ui.tokens().code_primitive_color
    };

    egui::RichText::new(datatype).color(color).into()
}

/// The columns whose display name or column name match, with the matched ranges of the display name.
fn matching_columns<'a>(
    matcher: &FilterMatcher,
    columns: &'a [FilterableColumn],
) -> Vec<(&'a FilterableColumn, PathRanges)> {
    columns
        .iter()
        .filter_map(|column| {
            let label = &column.display_name;
            let filter = &column.column_filter;
            let label_ranges = matcher.match_path([label.as_str()]);
            if label_ranges.is_none()
                && matcher.match_path([filter.field.name().as_str()]).is_none()
            {
                return None;
            }
            Some((column, label_ranges.unwrap_or_default()))
        })
        .collect()
}

/// The column picker behind "Add filter": a search field, the visible columns, and the hidden
/// columns below them on demand.
///
/// The search always covers the hidden columns too. `focus` grabs keyboard focus and clears the
/// search and the selection, pass it on the frame the popup opens.
fn column_picker_ui(
    ui: &mut egui::Ui,
    id: egui::Id,
    controls: &FilterControls,
    focus: bool,
) -> Option<ColumnFilter> {
    let search_id = id.with("search");
    let cursor_id = id.with("cursor");
    let keyboard_active_id = id.with("keyboard_active");
    let show_hidden_id = id.with("show_hidden");

    let mut search = if focus {
        String::new()
    } else {
        ui.data(|data| data.get_temp::<String>(search_id))
            .unwrap_or_default()
    };
    let mut cursor = ui
        .data(|data| data.get_temp::<usize>(cursor_id))
        .unwrap_or_default();
    let mut show_hidden = ui
        .data(|data| data.get_temp::<bool>(show_hidden_id))
        .unwrap_or_default();

    // Consume the navigation keys before the text edit uses them, so that the arrows move through
    // the list instead of the text, and Enter picks a column instead of defocusing the field.
    let has_focus = ui.memory(|memory| memory.has_focus(search_id));
    let (up, down, enter) = if has_focus {
        ui.input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                input.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            )
        })
    } else {
        (false, false, false)
    };

    let clear_id = egui::IdSalt::new("clear_search");
    let clear_button_size = Size::Tiny;
    let show_clear_button = !search.is_empty();
    let mut text_edit = ReTextEdit::singleline(&mut search)
        .id(search_id)
        .variant(TextEditVariant::Outlined)
        .prefix(icons::SEARCH)
        .hint_text("Column name")
        .desired_width(ui.available_width());
    if show_clear_button {
        text_edit = text_edit.suffix(Atom::custom(clear_id, clear_button_size.icon_button_size()));
    }
    let text_edit_response = text_edit.atom_ui(ui);

    let mut search_changed = false;
    if let Some(rect) = text_edit_response.rect(clear_id)
        && ui
            .place(
                rect,
                egui::Button::icon(icons::CLOSE, "Clear search").size(clear_button_size),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
    {
        search.clear();
        search_changed = true;
        text_edit_response.request_focus();
    }

    let response = text_edit_response.response;
    if focus {
        response.request_focus();
    }

    // A new query invalidates the selection, and hands control back to the mouse: the highlight
    // would otherwise sit on whatever row the previous selection happens to land on.
    if focus || search_changed || response.changed() {
        cursor = 0;
        ui.data_mut(|data| data.insert_temp(keyboard_active_id, false));
    } else if up || down {
        ui.data_mut(|data| data.insert_temp(keyboard_active_id, true));
    }
    let keyboard_active = ui
        .data(|data| data.get_temp::<bool>(keyboard_active_id))
        .unwrap_or_default();

    let query = search.trim();
    let searching = !query.is_empty();
    let mut filter_state = re_ui::filter_widget::FilterState::default();
    if searching {
        filter_state.activate(query);
    }
    let matcher = filter_state.filter();

    let visible = matching_columns(&matcher, &controls.columns);
    let hidden = if searching || show_hidden {
        matching_columns(&matcher, &controls.hidden_columns)
    } else {
        Vec::new()
    };
    let row_count = visible.len() + hidden.len();

    cursor = cursor.min(row_count.saturating_sub(1));
    if down {
        cursor = (cursor + 1).min(row_count.saturating_sub(1));
    }
    if up {
        cursor = cursor.saturating_sub(1);
    }

    let mut selected = if enter {
        std::iter::chain(&visible, &hidden)
            .nth(cursor)
            .map(|(column, _)| column.column_filter.clone())
    } else {
        None
    };

    let show_toggle = !searching && !controls.hidden_columns.is_empty();
    let mut toggle_clicked = false;
    let scroll = ui.spacing().scroll;
    let mut content_margin = scroll.content_margin;
    content_margin.right += (scroll.bar_width + scroll.bar_inner_margin) as i8;

    egui::ScrollArea::vertical()
        .id_salt(id)
        .max_height(COLUMN_PICKER_MAX_HEIGHT)
        .min_scrolled_height(COLUMN_PICKER_MAX_HEIGHT)
        .content_margin(content_margin)
        .show(ui, |ui| {
            ui.list_item_scope(id, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;

                let separator_at = (!searching && !visible.is_empty() && !hidden.is_empty())
                    .then_some(visible.len());

                for (row, (column, mut ranges)) in std::iter::chain(visible, hidden).enumerate() {
                    if Some(row) == separator_at {
                        let spacing = ui.spacing().item_spacing.y;
                        ui.add(egui::Separator::default().spacing(spacing));
                        ui.add_space(spacing);
                    }

                    let is_cursor_row = row == cursor;
                    let label = format_matching_text(
                        ui.ctx(),
                        &column.display_name,
                        ranges.remove(0).into_iter().flatten(),
                        &egui::TextStyle::Body,
                        None,
                    );
                    let filter = &column.column_filter;
                    let badge = datatype_badge(ui, filter);

                    let response = ui
                        .list_item()
                        .force_hovered(keyboard_active && is_cursor_row)
                        .show_flat(
                            ui,
                            CustomContent::new(|ui, _| {
                                egui::Sides::new()
                                    .height(ui.available_height())
                                    .shrink_left()
                                    .truncate()
                                    .show(
                                        ui,
                                        |ui| {
                                            ui.add(
                                                egui::Label::new(label)
                                                    .selectable(false)
                                                    .truncate(),
                                            )
                                        },
                                        |ui| ui.add(egui::Label::new(badge).selectable(false)),
                                    );
                            }),
                        )
                        .on_hover_text(filter.field.name());

                    if is_cursor_row && (up || down) {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }

                    if response.clicked() {
                        selected = Some(filter.clone());
                    }
                }

                if show_toggle {
                    let label = if show_hidden {
                        "Show less"
                    } else {
                        "Show hidden columns"
                    };
                    let color = ui.visuals().weak_text_color();
                    let mut arrow = icons::DROPDOWN_ARROW
                        .as_image()
                        .fit_to_exact_size(ui.tokens().small_icon_size)
                        .tint(color);
                    if show_hidden {
                        arrow = arrow.rotate(std::f32::consts::PI, egui::Vec2::splat(0.5));
                    }

                    toggle_clicked = ui
                        .list_item()
                        .show_flat(
                            ui,
                            CustomContent::new(|ui, _| {
                                ui.horizontal(|ui| {
                                    ui.add(
                                        egui::Label::new(egui::RichText::new(label).color(color))
                                            .selectable(false),
                                    );
                                    ui.add(arrow);
                                });
                            }),
                        )
                        .clicked();
                }
            });
        });

    if row_count == 0 {
        ui.weak("No matching columns");
    }

    if toggle_clicked {
        show_hidden = !show_hidden;
    }

    ui.data_mut(|data| {
        data.insert_temp(search_id, search);
        data.insert_temp(cursor_id, cursor);
        data.insert_temp(show_hidden_id, show_hidden);
    });
    selected
}

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

    pub fn clear(&mut self) {
        *self = Self::default();
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
    /// A `missing_sort_column` shows as an error pill with a button to remove the sort.
    ///
    /// Filters that select every row are left out of the returned filters.
    #[must_use]
    pub fn filter_bar_ui(
        &mut self,
        ui: &mut egui::Ui,
        timestamp_format: TimestampFormat,
        filter_errors: &FilterErrors,
        controls: Option<&FilterControls>,
        missing_sort_column: Option<&ColumnName>,
    ) -> FilterBarOutput {
        // From there on, we always want to show the "today" date, because not doing so leads
        // to some very confusing display.
        let timestamp_format =
            timestamp_format.with_date_visibility(re_log_types::DateVisibility::ShowDate);

        self.popup_open = false;

        if self.filters.is_empty() && controls.is_none() && missing_sort_column.is_none() {
            return Default::default();
        }

        let mut added_filter = None;
        let mut action = FilterUiAction::None;
        let mut remove_sort = false;

        Frame::new()
            .inner_margin(Margin {
                top: if controls.is_some() { 0 } else { 16 },
                bottom: 12,
                left: 16,
                right: 16,
            })
            .show(ui, |ui| {
                let active_index = self.active_filter.take();
                let mut remove_idx = None;

                ui.horizontal_wrapped(|ui| {
                    if let Some(column) = missing_sort_column {
                        remove_sort = missing_sort_pill_ui(ui, column);
                    }

                    if controls.is_some() && self.filters.len() > 1 {
                        ui.add_sized(
                            [0.0, ui.text_style_height(&egui::TextStyle::Body) + 8.0],
                            egui::Label::new("Match all"),
                        )
                        .on_hover_text("Rows must match every filter (AND).");
                    }
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
                                    controls,
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

                    if let Some(controls) = controls {
                        let popup_id = ui.make_persistent_id("add_filter_menu");
                        let response = ui.add(
                            egui::Button::new(("Add filter", icons::FILTER))
                                .size_small()
                                .highlighted(egui::Popup::is_id_open(ui.ctx(), popup_id)),
                        );

                        egui::Popup::menu(&response)
                            .id(popup_id)
                            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                            .show(|ui| {
                                ui.set_width(COLUMN_PICKER_WIDTH);

                                if let Some(filter) =
                                    column_picker_ui(ui, popup_id, controls, response.clicked())
                                {
                                    added_filter = Some(filter);
                                    ui.close();
                                }
                            });
                    }

                    if let Some(remove_idx) = remove_idx {
                        self.filters.remove(remove_idx);
                    }

                    if controls.is_some()
                        && !self.filters.is_empty()
                        && ui
                            .add(egui::Button::new("Clear filters").size_small())
                            .clicked()
                    {
                        self.clear();
                        added_filter = None;
                        action = FilterUiAction::CommitStateToBlueprint;
                    }
                });
            });

        if let Some(filter) = added_filter {
            self.push_new_filter(filter);
            ui.ctx().request_repaint();
        }

        // On cancel the state isn't stored, so the next frame reads the blueprint again.
        if action != FilterUiAction::CommitStateToBlueprint {
            return FilterBarOutput {
                filters: None,
                remove_sort,
            };
        }

        // give a chance to filters to clean themselves up before committing to the table
        // blueprint
        for filter in &mut self.filters {
            if let TableFilter::Column(column_filter) = filter {
                column_filter.filter.on_commit();
            }
        }
        FilterBarOutput {
            filters: Some(
                self.filters
                    .iter()
                    .filter_map(TableFilter::to_sql)
                    .collect(),
            ),
            remove_sort,
        }
    }
}

/// What the user changed in the filter bar.
#[derive(Debug, Default)]
pub struct FilterBarOutput {
    /// The filters to write to the table blueprint when the user commits an edit.
    pub filters: Option<Vec<String>>,

    /// Whether the user removed the sort on a missing column.
    pub remove_sort: bool,
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
        controls: Option<&FilterControls>,
    ) -> DisplayFilterUiResult {
        let display_name = controls
            .and_then(|controls| {
                std::iter::chain(&controls.columns, &controls.hidden_columns)
                    .find(|column| column.column_filter.field.name() == self.field.name())
                    .map(|column| column.display_name.as_str())
            })
            .unwrap_or(self.field.name());
        let layout_job = SyntaxHighlightedBuilder::new()
            .with_body_default(display_name)
            .with_keyword(" ")
            .with(&TimestampFormatted::new(&self.filter, timestamp_format))
            .to_job(ui.style());

        let (response, should_delete_filter) =
            filter_pill_ui(ui, None, layout_job, "Remove filter");
        let response = response.on_hover_text(self.field.name());
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
            let action = self
                .filter
                .popup_ui(ui, timestamp_format, display_name, popup_was_closed);

            // Ensure we close the popup if the popup ui decided on an action.
            if action != FilterUiAction::None {
                ui.close();
            }

            action
        });

        if popup_response
            .as_ref()
            .is_some_and(|popup| popup.response.should_close())
        {
            response.request_focus();
        }
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
/// `remove_label` is the label of the remove button.
///
/// Returns the pill's response, and whether the remove button was clicked.
fn filter_pill_ui(
    ui: &mut egui::Ui,
    icon: Option<egui::Image<'static>>,
    layout_job: egui::text::LayoutJob,
    remove_label: &str,
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
        .stroke(ui.tokens().control_stroke)
        .corner_radius(2.0);

    let atom_layout = WidgetAtom::new(atoms).sense(Sense::click()).frame(frame);

    let atom_response = atom_layout.show(ui);

    let should_delete_filter = atom_response
        .rect(ColumnFilter::close_button_id())
        .is_some_and(|rect| {
            ui.place(
                rect,
                ui.small_icon_button_widget(&re_ui::icons::CLOSE_SMALL, remove_label)
                    .size(Size::custom(rect.height())),
            )
            .clicked()
        });

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
    let has_shown_custom_filter_id = egui::Id::unique("has_shown_custom_table_filter");
    let has_shown_custom_filter = ui.data_mut(|data| {
        std::mem::replace(
            data.get_temp_mut_or_default::<bool>(has_shown_custom_filter_id),
            true,
        )
    });
    if !has_shown_custom_filter {
        re_analytics::record(|| re_analytics::event::CustomTableFilterShown {});
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
            error_text(ui, &text)
        }
    }
    .to_job(ui.style());

    let icon = error.map(|_| error_icon(ui));

    let (response, should_delete_filter) = filter_pill_ui(ui, icon, layout_job, "Remove filter");
    response.on_hover_ui(|ui| {
        let theme = egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style());
        egui_extras::syntax_highlighting::code_view_ui(ui, &theme, &custom_filter.sql, "sql");
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

/// UI for a sort on a column the table doesn't have, in the error color with an error icon.
///
/// Returns whether the user removed the sort.
fn missing_sort_pill_ui(ui: &mut egui::Ui, column: &ColumnName) -> bool {
    let column = column.as_str();
    let layout_job = error_text(ui, &format!(r#"Can't sort by "{column}""#)).to_job(ui.style());
    let icon = error_icon(ui);

    let (response, should_remove_sort) = filter_pill_ui(ui, Some(icon), layout_job, "Remove sort");
    response.on_hover_ui(|ui| {
        ui.error_label(format!(
            r#"The table has no column "{column}", so the rows are not sorted"#
        ));
    });

    should_remove_sort
}

fn error_text(ui: &egui::Ui, text: &str) -> SyntaxHighlightedBuilder {
    SyntaxHighlightedBuilder::new().with_format(
        text,
        egui::TextFormat::simple(
            egui::TextStyle::Body.resolve(ui.style()),
            ui.visuals().error_fg_color,
        ),
    )
}

fn error_icon(ui: &egui::Ui) -> egui::Image<'static> {
    re_ui::icons::ERROR
        .as_image()
        .tint(ui.tokens().alert_error.icon)
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
    use egui_kittest::kittest::Queryable as _;

    use super::super::{
        ComparisonOperator, FloatFilter, IntFilter, NonNullableBooleanFilter,
        NullableBooleanFilter, StringFilter, StringOperator, TimestampFilter, TypedFilter,
    };
    use super::*;

    fn int_column(label: &str, name: &str) -> FilterableColumn {
        FilterableColumn {
            display_name: label.to_owned(),
            column_filter: ColumnFilter::new(
                dummy_field(name),
                IntFilter::new(ComparisonOperator::Eq, None),
            ),
        }
    }

    fn picked_column(
        controls: &FilterControls,
        interact: impl FnOnce(&mut egui_kittest::Harness<'_>),
    ) -> Option<String> {
        let mut selected = None;
        let mut first_frame = true;
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(400.0, 600.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                if let Some(filter) = column_picker_ui(
                    ui,
                    egui::Id::unique("column_picker_test"),
                    controls,
                    first_frame,
                ) {
                    selected = Some(filter.field.name().clone());
                }
                first_frame = false;
            });
        harness.run();
        interact(&mut harness);
        drop(harness);
        selected
    }

    #[test]
    fn column_picker_keyboard_picks_the_highlighted_column() {
        let controls = FilterControls {
            columns: vec![
                int_column("Field 1", "field_1"),
                int_column("Field 2", "field_2"),
                int_column("Other", "other"),
            ],
            hidden_columns: Vec::new(),
        };
        let selected = picked_column(&controls, |harness| {
            harness
                .get_by_role(egui::accesskit::Role::TextInput)
                .type_text("field");
            harness.run();
            harness.key_press(egui::Key::ArrowDown);
            harness.run();
            harness.key_press(egui::Key::Enter);
            harness.run();
        });
        assert_eq!(selected.as_deref(), Some("field_2"));
    }

    #[test]
    fn column_picker_searches_collapsed_hidden_columns() {
        let controls = FilterControls {
            columns: vec![int_column("Visible", "visible")],
            hidden_columns: vec![int_column("Hidden", "hidden")],
        };
        let selected = picked_column(&controls, |harness| {
            assert!(harness.query_by_label("Hidden").is_none());
            harness
                .get_by_role(egui::accesskit::Role::TextInput)
                .type_text("hid");
            harness.run();
            assert!(harness.query_by_label("Visible").is_none());
            harness.get_by_label("Hidden").click();
            harness.run();
        });
        assert_eq!(selected.as_deref(), Some("hidden"));
    }

    #[test]
    fn add_filter_keyboard_selection_can_be_cancelled() {
        let controls = FilterControls {
            columns: vec![int_column("Success", "success")],
            hidden_columns: Vec::new(),
        };
        let schema = Schema::empty();
        let state_id = egui::Id::unique("filter_bar_test");
        let mut saved: Vec<String> = Vec::new();
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                let mut filters = FilterState::load(ui.ctx(), state_id, &saved, &schema);
                if let Some(new_filters) = filters
                    .filter_bar_ui(
                        ui,
                        TimestampFormat::utc(),
                        &Default::default(),
                        Some(&controls),
                        None,
                    )
                    .filters
                {
                    saved = new_filters;
                }
                filters.store(ui.ctx(), state_id);
            });
        harness.run();
        harness.get_by_label("Add filter").click();
        harness.run();
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .type_text("Success");
        harness.run();
        harness.key_press(egui::Key::Enter);
        harness.run();
        harness.key_press(egui::Key::Escape);
        harness.run();
        assert!(harness.query_by_label("Remove filter").is_none());
        drop(harness);
        assert!(saved.is_empty());
    }

    #[test]
    fn hidden_columns_can_be_revealed_and_filtered() {
        let filter = ColumnFilter::new(
            dummy_field("hidden"),
            IntFilter::new(ComparisonOperator::Eq, None),
        );
        let controls = FilterControls {
            columns: Vec::new(),
            hidden_columns: vec![FilterableColumn {
                display_name: "Hidden".to_owned(),
                column_filter: filter.clone(),
            }],
        };
        let mut filters = FilterState::default();
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(800.0, 600.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                let _filters_to_save = filters.filter_bar_ui(
                    ui,
                    TimestampFormat::utc(),
                    &Default::default(),
                    Some(&controls),
                    None,
                );
            });
        harness.run();
        harness.get_by_label("Add filter").click();
        harness.run();
        assert!(harness.query_by_label("Hidden").is_none());
        harness.get_by_label("Show hidden columns").click();
        harness.run();
        harness.get_by_label("Hidden").click();
        harness.run();
        drop(harness);
        assert_eq!(filters.filters, vec![TableFilter::Column(filter)]);
    }

    #[test]
    fn clearing_search_restores_columns() {
        let controls = FilterControls {
            columns: vec![int_column("First", "first"), int_column("Second", "second")],
            hidden_columns: Vec::new(),
        };
        picked_column(&controls, |harness| {
            harness
                .get_by_role(egui::accesskit::Role::TextInput)
                .type_text("Second");
            harness.run();
            assert!(harness.query_by_label("First").is_none());
            harness.get_by_label("Clear search").click();
            harness.run();
            assert!(harness.query_by_label("First").is_some());
        });
    }

    #[test]
    fn clearing_filter_state_resets_its_edit_state() {
        let mut state = FilterState::new(vec![TableFilter::Column(ColumnFilter::new(
            dummy_field("field"),
            IntFilter::new(ComparisonOperator::Eq, None),
        ))]);
        state.active_filter = Some(0);
        state.popup_open = true;

        state.clear();

        assert!(state.filters.is_empty());
        assert_eq!(state.active_filter, None);
        assert!(!state.popup_open);
    }

    #[test]
    fn removing_one_filter_preserves_the_other() {
        let first = ColumnFilter::new(
            dummy_field("first"),
            IntFilter::new(ComparisonOperator::Eq, Some(1)),
        );
        let second = ColumnFilter::new(
            dummy_field("second"),
            IntFilter::new(ComparisonOperator::Eq, Some(2)),
        );
        let mut saved = None;
        let mut filters = FilterState::new(vec![
            TableFilter::Column(first.clone()),
            TableFilter::Column(second.clone()),
        ]);
        let controls = FilterControls {
            columns: vec![FilterableColumn {
                display_name: "First".to_owned(),
                column_filter: first,
            }],
            hidden_columns: Vec::new(),
        };
        let mut harness = egui_kittest::Harness::builder()
            .with_size(egui::vec2(800.0, 200.0))
            .build_ui(|ui| {
                re_ui::apply_style_and_install_loaders(ui.ctx());
                if let Some(new_filters) = filters
                    .filter_bar_ui(
                        ui,
                        TimestampFormat::utc(),
                        &Default::default(),
                        Some(&controls),
                        None,
                    )
                    .filters
                {
                    saved = Some(new_filters);
                }
            });
        harness.run();
        harness
            .query_all_by_label("Remove filter")
            .next()
            .unwrap()
            .click();
        harness.run();
        drop(harness);
        let second = TableFilter::Column(second);
        assert_eq!(saved, Some(second.to_sql().into_iter().collect()));
        assert_eq!(filters.filters, vec![second]);
    }

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
                .with_context_setup(re_ui::apply_style_and_install_loaders)
                .with_size(egui::Vec2::new(800.0, 80.0))
                .build_ui(|ui| {
                    let mut filter_state = FilterState::new(vec![TableFilter::Column(
                        ColumnFilter::new(dummy_field("column:name"), filter.clone()),
                    )]);

                    let _res = filter_state.filter_bar_ui(
                        ui,
                        TimestampFormat::utc(),
                        &Default::default(),
                        None,
                        None,
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
                .with_context_setup(re_ui::apply_style_and_install_loaders)
                .with_size(egui::Vec2::new(400.0, 400.0))
                .build_ui(|ui| {
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
            .with_context_setup(re_ui::apply_style_and_install_loaders)
            .with_size(egui::Vec2::new(700.0, 500.0))
            .build_ui(|ui| {
                let _filters_to_save = filters.filter_bar_ui(
                    ui,
                    TimestampFormat::utc(),
                    &Default::default(),
                    None,
                    None,
                );
            });

        harness.run();

        harness.snapshot("filter_wrapping");
    }

    #[test]
    fn test_add_filter_column_picker() {
        fn column(display_name: &str, name: &str, datatype: DataType) -> FilterableColumn {
            FilterableColumn {
                display_name: display_name.to_owned(),
                column_filter: ColumnFilter::default_for_column(Arc::new(Field::new(
                    name, datatype, true,
                )))
                .expect("column type is filterable"),
            }
        }

        let controls = FilterControls {
            columns: vec![
                column("Recording link", "recording_link", DataType::Utf8),
                column("Attachment count", "attachment_count", DataType::Int64),
                column("Duration", "duration", DataType::Float64),
                column("Success", "success", DataType::Boolean),
                column(
                    "Start time",
                    "start_time",
                    DataType::Timestamp(arrow::datatypes::TimeUnit::Nanosecond, Some("UTC".into())),
                ),
            ],
            hidden_columns: vec![
                column("Channel count", "channel_count", DataType::Int64),
                column("Robot name", "robot_name", DataType::Utf8),
            ],
        };
        let mut filters = FilterState::default();
        let mut harness = egui_kittest::Harness::builder()
            .with_context_setup(re_ui::apply_style_and_install_loaders)
            .with_size(egui::Vec2::new(320.0, 330.0))
            .build_ui(|ui| {
                let _filters_to_save = filters.filter_bar_ui(
                    ui,
                    TimestampFormat::utc(),
                    &Default::default(),
                    Some(&controls),
                    None,
                );
            });
        let mut snapshot_results = SnapshotResults::new();

        harness.run();
        harness.get_by_label("Add filter").click();
        harness.run();
        harness.remove_cursor();
        harness.run();
        snapshot_results.add(harness.try_snapshot_options(
            "add_filter_picker_open",
            &re_ui::testing::default_snapshot_options_for_ui(),
        ));

        harness.get_by_label("Show hidden columns").click();
        harness.run();
        harness.remove_cursor();
        harness.run();
        snapshot_results.add(harness.try_snapshot_options(
            "add_filter_picker_hidden_shown",
            &re_ui::testing::default_snapshot_options_for_ui(),
        ));

        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .click();
        harness.run();
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .type_text("count");
        harness.run();
        harness.remove_cursor();
        harness.run();
        snapshot_results.add(harness.try_snapshot_options(
            "add_filter_picker_search",
            &re_ui::testing::default_snapshot_options_for_ui(),
        ));
    }
}
