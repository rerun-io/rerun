use ahash::HashSet;
use egui::containers::menu::{MenuButton, MenuConfig};
use egui::emath::GuiRounding as _;
use egui::{Color32, Frame, Id, PopupCloseBehavior, RichText, Stroke, Style};
use re_sdk_types::blueprint::components::ColumnName;
use re_ui::{UiExt as _, design_tokens_of, icons};

use crate::datafusion_table_widget::Columns;

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
}

impl UiTableConfig {
    fn new(id: Id) -> Self {
        Self {
            id,
            columns: Vec::new(),
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

            config.clone()
        })
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

    fn ui(&mut self, ui: &mut egui::Ui, columns: &Columns<'_>) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            let response = egui_dnd::dnd(ui, "Columns").show(
                self.columns.iter_mut(),
                |ui, ui_column, handle, _state| {
                    let visible = ui_column.visible;
                    egui::Sides::new().show(
                        ui,
                        |ui| {
                            handle.ui(ui, |ui| {
                                ui.small_icon(&icons::DND_HANDLE, Some(ui.visuals().text_color()));
                            });
                            let mut label = RichText::new(
                                columns
                                    .find_by_physical_name(&ui_column.physical_name)
                                    .map(|(_, column)| column.display_name())
                                    .unwrap_or_else(|| ui_column.physical_name.as_str().to_owned()),
                            );
                            if visible {
                                label = label.strong();
                            } else {
                                label = label.weak();
                            }
                            ui.label(label);
                        },
                        |ui| {
                            let (icon, alt_text) = if ui_column.visible {
                                (&icons::VISIBLE, "Hide column")
                            } else {
                                (&icons::INVISIBLE, "Show column")
                            };
                            if ui.small_icon_button(icon, alt_text).clicked() {
                                ui_column.visible = !ui_column.visible;
                            }
                        },
                    );
                },
            );
            if response.is_drag_finished() {
                response.update_vec(self.columns.as_mut_slice());
            }
        });
    }

    pub fn button_ui(&mut self, ui: &mut egui::Ui, columns: &Columns<'_>) {
        MenuButton::from_button(icons::TABLE_COLUMNS.as_button_with_label(ui.tokens(), "Columns"))
            .config(MenuConfig::new().close_behavior(PopupCloseBehavior::CloseOnClickOutside))
            .ui(ui, |ui| {
                self.ui(ui, columns);
            });
    }
}
