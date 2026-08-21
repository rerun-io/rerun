use ahash::HashSet;
use egui::containers::menu::{MenuButton, MenuConfig};
use egui::emath::GuiRounding as _;
use egui::{Color32, Frame, Id, Label, Link, PopupCloseBehavior, RichText, Stroke, Style};
use re_sdk_types::blueprint::components::ColumnName;
use re_ui::text_edit::{ReTextEdit, TextEditVariant};
use re_ui::{UiExt as _, design_tokens_of, icons};

use crate::datafusion_table_widget::{Column, Columns};

pub const CELL_SEPARATOR_STROKE_OFFSET: f32 = 0.5;

/// This applies some fixes so that the column resize bar is correctly displayed.
///
/// Remember to revert the styling within the cells!
pub fn apply_table_style_fixes(style: &mut Style) {
    let theme = if style.visuals.dark_mode {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };

    let design_tokens = design_tokens_of(theme);

    style.visuals.widgets.hovered.bg_stroke =
        Stroke::new(1.0, design_tokens.table_interaction_hovered_bg_stroke);
    style.visuals.widgets.active.bg_stroke =
        Stroke::new(1.0, design_tokens.table_interaction_active_bg_stroke);
    // regular vertical lines are drawn in cell_ui to allow cells to be connected
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(0.0, Color32::TRANSPARENT);
}

pub fn header_ui<R>(
    ui: &mut egui::Ui,
    table_style: re_ui::TableStyle,
    connected_to_next_cell: bool,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let rect = ui
        .max_rect()
        .round_to_pixels(ui.pixels_per_point())
        .round_ui();

    ui.painter()
        .rect_filled(rect, 0.0, ui.tokens().table_header_bg_fill);

    let response = Frame::new()
        .inner_margin(ui.tokens().header_cell_margin(table_style))
        .show(ui, content);

    if !connected_to_next_cell {
        ui.painter().vline(
            rect.max.x - CELL_SEPARATOR_STROKE_OFFSET,
            rect.y_range(),
            Stroke::new(1.0, ui.tokens().table_header_stroke_color),
        );
    }

    ui.painter().hline(
        rect.x_range(),
        rect.max.y - CELL_SEPARATOR_STROKE_OFFSET, // - 1.0 prevents it from being overdrawn by the following row
        Stroke::new(1.0, ui.tokens().table_header_stroke_color),
    );

    response
}

pub fn cell_ui<R>(
    ui: &mut egui::Ui,
    table_style: re_ui::TableStyle,
    connected_to_next_cell: bool,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let response = Frame::new()
        .inner_margin(ui.tokens().table_cell_margin(table_style))
        .show(ui, content);

    let rect = ui
        .max_rect()
        .round_to_pixels(ui.pixels_per_point())
        .round_ui();

    if !connected_to_next_cell {
        ui.painter().vline(
            rect.max.x - CELL_SEPARATOR_STROKE_OFFSET,
            rect.y_range(),
            Stroke::new(1.0, ui.tokens().table_interaction_noninteractive_bg_stroke),
        );
    }

    ui.painter().hline(
        rect.x_range(),
        rect.max.y - CELL_SEPARATOR_STROKE_OFFSET, // - 1.0 prevents it from being overdrawn by the following row
        Stroke::new(1.0, ui.tokens().table_interaction_noninteractive_bg_stroke),
    );

    response
}

fn default_column_name() -> ColumnName {
    "".into()
}

/// Column configuration stored in egui state.
#[derive(Debug, Clone, Hash, serde::Serialize, serde::Deserialize)]
pub struct UiColumnConfig {
    /// The original Arrow field name used by DataFusion.
    #[serde(alias = "column_name", default = "default_column_name")]
    physical_name: ColumnName,

    visible: bool,
    sort_key: i64,
}

impl UiColumnConfig {
    fn new(physical_name: ColumnName, visible: bool) -> Self {
        Self {
            physical_name,
            visible,
            sort_key: 0,
        }
    }

    /// Set a sort key. This will affect the order of new columns added to the table.
    ///
    /// Default is 0.
    fn with_sort_key(mut self, sort_key: i64) -> Self {
        self.sort_key = sort_key;
        self
    }

    pub(crate) fn column_name(&self) -> &ColumnName {
        &self.physical_name
    }
}

// TODO(lucasmerlin): It would be nice to have this in egui_table, so egui_table could do the work
// of showing / hiding columns based on the config.
// https://github.com/rerun-io/egui_table/issues/27
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UiTableConfig {
    id: Id,
    columns: Vec<UiColumnConfig>,
    #[serde(skip)]
    filter: String,
}

impl UiTableConfig {
    fn new(id: Id) -> Self {
        Self {
            id,
            columns: Vec::new(),
            filter: String::new(),
        }
    }

    /// Get a table config, creating it if it doesn't exist.
    ///
    /// Loads the config from egui state and merges it with the provided columns and their blueprints.
    /// This preserves existing state and adds missing columns from the provided list.
    ///
    /// Don't forget to call [`Self::store`] to persist the changes.
    pub fn from_egui_state_merged_with_data_columns(
        egui_ctx: &egui::Context,
        persisted_id: Id,
        columns: &Columns<'_>,
    ) -> Self {
        egui_ctx.data_mut(|data| {
            let config: &mut Self =
                data.get_persisted_mut_or_insert_with(persisted_id, || Self::new(persisted_id));

            let mut present_columns = HashSet::default();
            let mut new_columns = Vec::new();

            for column in columns.iter() {
                present_columns.insert(column.physical_name());
                if config
                    .columns
                    .iter()
                    .all(|c| &c.physical_name != column.physical_name())
                {
                    new_columns.push(
                        UiColumnConfig::new(
                            column.physical_name().clone(),
                            column.blueprint.default_visibility,
                        )
                        .with_sort_key(column.blueprint.sort_key),
                    );
                }
            }

            new_columns.sort_by_key(|column| column.sort_key);
            config.columns.extend(new_columns);

            config
                .columns
                .retain(|column| present_columns.contains(&column.physical_name));

            config.sort_visible_first();

            config.clone()
        })
    }

    /// Move all visible columns before the hidden ones, keeping their relative order.
    fn sort_visible_first(&mut self) {
        self.columns.sort_by_key(|column| !column.visible);
    }

    /// At which index will the "Hidden columns" header be shown?
    ///
    /// The header goes right before the first hidden column, or last if every column is shown.
    fn hidden_header_index(&self) -> HiddenHeaderIndex {
        HiddenHeaderIndex(
            self.columns
                .iter()
                .position(|column| !column.visible)
                .unwrap_or(self.columns.len()),
        )
    }

    pub fn store(self, egui_ctx: &egui::Context) {
        egui_ctx.data_mut(|data| {
            data.insert_persisted(self.id, self);
        });
    }

    pub fn visible_columns(&self) -> impl Iterator<Item = &UiColumnConfig> {
        self.columns.iter().filter(|col| col.visible)
    }

    pub fn visible_column_names(&self) -> impl Iterator<Item = &ColumnName> {
        self.visible_columns().map(|column| column.column_name())
    }

    /// Apply a finished drag, with `from` and `to` being entry indices of the column list.
    ///
    /// The header item can't be dragged, so a drag always moves a single column.
    /// That dragged column can change visibility, if it is dragged above or below the "hidden"
    /// header.
    fn apply_entry_drag(
        &mut self,
        hidden_header_index: HiddenHeaderIndex,
        from: EntryIndex,
        to: EntryIndex,
    ) {
        let Some(column_from) = hidden_header_index.column_index(from) else {
            return;
        };
        let column_to = hidden_header_index.columns_before(to);

        if column_from.0 >= self.columns.len() || column_to.0 > self.columns.len() {
            // Out of range for `shift_vec`, which would panic.
            return;
        }

        // Set the visibility before shifting via the `from` index
        self.columns[column_from.0].visible = to <= hidden_header_index.entry();

        egui_dnd::utils::shift_vec(column_from.0, column_to.0, &mut self.columns);
    }

    fn ui(&mut self, ui: &mut egui::Ui, columns: &Columns<'_>) {
        ui.add(
            ReTextEdit::singleline(&mut self.filter)
                .prefix(icons::SEARCH)
                .variant(TextEditVariant::Outlined)
                .hint_text("Column name"),
        );
        let filter = self.filter.trim().to_lowercase();

        // A lookup table, so that resolving the display names below is linear and not quadratic.
        let columns_by_name: ahash::HashMap<&ColumnName, &Column<'_>> = columns
            .iter()
            .map(|column| (column.physical_name(), column))
            .collect();

        // The display name of each column, and whether the filter lets it through.
        // Indexed like `self.columns`.
        let entries: Vec<(String, bool)> = self
            .columns
            .iter()
            .map(|column_config| {
                let display_name = columns_by_name
                    .get(&column_config.physical_name)
                    .map_or_else(
                        || column_config.physical_name.as_str().to_owned(),
                        |column| column.display_name(),
                    );
                let listed = filter.is_empty()
                    || display_name.to_lowercase().contains(&filter)
                    || column_config
                        .physical_name
                        .as_str()
                        .to_lowercase()
                        .contains(&filter);
                (display_name, listed)
            })
            .collect();

        let hidden_header_index = self.hidden_header_index();

        // The visible columns come first, so the two sections are the two halves of `entries`.
        let (shown, hidden) = entries.split_at(hidden_header_index.shown_count());
        let listed_shown_count = shown.iter().filter(|(_, listed)| *listed).count();
        let listed_hidden_count = hidden.iter().filter(|(_, listed)| *listed).count();

        let mut set_all_visible = None;
        let mut toggled_column = None;

        let dnd_response = egui::ScrollArea::vertical()
            .min_scrolled_height(400.0)
            .show(ui, |ui| {
                // The "shown" header is not part of dnd, since it doesn't make sense to move
                // something above it.
                if section_header_ui(
                    ui,
                    &format!("Shown in table ({listed_shown_count})"),
                    "Hide all",
                    listed_shown_count,
                ) {
                    set_all_visible = Some(false);
                }

                egui_dnd::dnd(ui, "columns").show_custom(|ui, iter| {
                    // 1 extra entry for the "hidden" header
                    let entry_count = self.columns.len() + 1;

                    for entry_index in (0..entry_count).map(EntryIndex) {
                        if let Some(column_index) = hidden_header_index.column_index(entry_index) {
                            let (display_name, listed) = &entries[column_index.0];
                            if !listed {
                                continue;
                            }

                            if column_row_ui(
                                ui,
                                iter,
                                entry_index,
                                &self.columns[column_index.0],
                                display_name,
                            ) {
                                toggled_column = Some(column_index);
                            }
                        } else {
                            // The "hidden" header is part of dnd, so a column can be dropped
                            // above or below it.
                            ui.add_space(8.0);
                            let id = Id::new("hidden_column_section");
                            iter.next(ui, id, entry_index.0, true, |ui, item| {
                                item.ui(ui, |ui, _handle, _state| {
                                    if section_header_ui(
                                        ui,
                                        &format!("Hidden in table ({listed_hidden_count})"),
                                        "Show all",
                                        listed_hidden_count,
                                    ) {
                                        set_all_visible = Some(true);
                                    }
                                })
                            });
                        }
                    }
                })
            })
            .inner;

        // Apply the drag before any visibility change, so that `hidden_header_index` is still accurate.
        if let Some(update) = dnd_response.final_update() {
            self.apply_entry_drag(
                hidden_header_index,
                EntryIndex(update.from),
                EntryIndex(update.to),
            );
        }

        if let Some(visible) = set_all_visible {
            for (column, (_, listed)) in std::iter::zip(&mut self.columns, &entries) {
                if *listed {
                    column.visible = visible;
                }
            }
        }

        if let Some(column_index) = toggled_column {
            let column = &mut self.columns[column_index.0];
            column.visible = !column.visible;
        }

        self.sort_visible_first();
    }

    pub fn button_ui(&mut self, ui: &mut egui::Ui, columns: &Columns<'_>) {
        MenuButton::from_button(icons::TABLE_COLUMNS.as_button_with_label(ui.tokens(), "Columns"))
            .config(MenuConfig::new().close_behavior(PopupCloseBehavior::CloseOnClickOutside))
            .ui(ui, |ui| {
                self.ui(ui, columns);
            });
    }
}

/// The position of an item in the drag-and-drop list.
///
/// The list has one item per column, plus the "Hidden columns" header, so it is one longer than
/// [`UiTableConfig::columns`]. Use [`HiddenHeaderIndex`] to convert to a [`ColumnIndex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct EntryIndex(usize);

/// The position of a column in [`UiTableConfig::columns`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ColumnIndex(usize);

/// The index that splits between the visible and hidden columns.
///
/// This helper exists to deal with the index offset introduced by the "Hidden columns" header item.
/// It's a separate item within the list so that you can drag items both above (making them visible)
/// or below (hiding them).
/// The first hidden entry comes right after [`Self::entry`].
#[derive(Debug, Clone, Copy)]
struct HiddenHeaderIndex(usize);

impl HiddenHeaderIndex {
    /// Where the "hidden" header sits in the drag-and-drop list.
    fn entry(self) -> EntryIndex {
        EntryIndex(self.0)
    }

    /// How many columns are listed above the header, i.e. where [`UiTableConfig::columns`] splits.
    fn shown_count(self) -> usize {
        self.0
    }

    /// The column an entry shows, or `None` for the "hidden" header.
    ///
    /// Helper to deal with the offset introduced by the "hidden" header.
    fn column_index(self, entry_index: EntryIndex) -> Option<ColumnIndex> {
        match entry_index.cmp(&self.entry()) {
            std::cmp::Ordering::Less => Some(ColumnIndex(entry_index.0)),
            std::cmp::Ordering::Equal => None,
            std::cmp::Ordering::Greater => Some(ColumnIndex(entry_index.0 - 1)),
        }
    }

    /// How many columns are listed before `entry_index`.
    fn columns_before(self, entry_index: EntryIndex) -> ColumnIndex {
        ColumnIndex(entry_index.0 - usize::from(entry_index > self.entry()))
    }
}

/// Header of a column section, with a link to show or hide all its listed columns.
///
/// The link is disabled when the section lists no columns.
///
/// Returns `true` if the link was clicked.
fn section_header_ui(ui: &mut egui::Ui, title: &str, action_label: &str, count: usize) -> bool {
    egui::Sides::new()
        .shrink_left()
        .show(
            ui,
            |ui| {
                ui.add(Label::new(RichText::new(title).strong()));
            },
            |ui| {
                ui.add_enabled(
                    count > 0,
                    Link::new(RichText::new(action_label).color(ui.tokens().selection_bg_fill)),
                )
                .clicked()
            },
        )
        .1
}

/// A single draggable column row: drag handle, column name, and a button to show or hide it.
///
/// Returns `true` if the show/hide button was clicked.
fn column_row_ui(
    ui: &mut egui::Ui,
    iter: &mut egui_dnd::ItemIterator<'_>,
    entry_index: EntryIndex,
    column: &UiColumnConfig,
    label: &str,
) -> bool {
    // The physical name identifies the column; the label is only what the user reads.
    let id = Id::new(("column", &column.physical_name));

    let mut clicked = false;

    iter.next(ui, id, entry_index.0, true, |ui, item| {
        item.ui(ui, |ui, handle, _state| {
            egui::Sides::new().shrink_left().truncate().show(
                ui,
                |ui| {
                    handle.ui(ui, |ui| {
                        ui.small_icon(&icons::DND_HANDLE, Some(ui.visuals().text_color()));
                    });

                    let label = RichText::new(label);
                    ui.label(if column.visible {
                        label.strong()
                    } else {
                        label.weak()
                    });
                },
                |ui| {
                    let (icon, alt_text) = if column.visible {
                        (&icons::VISIBLE, "Hide column")
                    } else {
                        (&icons::INVISIBLE, "Show column")
                    };
                    clicked = ui.small_icon_button(icon, alt_text).clicked();
                },
            );
        })
    });

    clicked
}
