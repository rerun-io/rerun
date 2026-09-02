use egui::containers::menu::{MenuButton, MenuConfig};
use egui::emath::GuiRounding as _;
use egui::{Color32, Frame, PopupCloseBehavior, RichText, Stroke, Style};
use re_sdk_types::blueprint::components::TableLayoutKind;
use re_ui::{UiExt as _, design_tokens_of, icons};
use re_viewer_context::AppBlueprintCtx;

use crate::blueprint::TableBlueprint;
use crate::blueprint::TableColumn;

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

#[derive(Debug, Clone)]
struct DndColumn<'a>(&'a TableColumn<'a>);

impl std::hash::Hash for DndColumn<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(self.0.physical_name(), state);
    }
}

pub fn columns_edit_menu_ui<'a>(
    ui: &mut egui::Ui,
    blueprint_ctx: &AppBlueprintCtx<'_>,
    layout_kind: TableLayoutKind,
    columns: impl Iterator<Item = &'a TableColumn<'a>>,
) {
    MenuButton::from_button(icons::TABLE_COLUMNS.as_button_with_label(ui.tokens(), "Columns"))
        .config(MenuConfig::new().close_behavior(PopupCloseBehavior::CloseOnClickOutside))
        .ui(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut columns = columns.map(DndColumn).collect::<Vec<_>>();
                let response = egui_dnd::dnd(ui, "Columns").show(
                    columns.iter(),
                    |ui, column, handle, _state| {
                        let visible = column.0.is_visible(layout_kind);
                        egui::Sides::new().show(
                            ui,
                            |ui| {
                                handle.ui(ui, |ui| {
                                    ui.small_icon(
                                        &icons::DND_HANDLE,
                                        Some(ui.visuals().text_color()),
                                    );
                                });
                                let mut label = RichText::new(column.0.display_name());
                                if visible {
                                    label = label.strong();
                                } else {
                                    label = label.weak();
                                }
                                ui.label(label);
                            },
                            |ui| {
                                let (icon, alt_text) = if visible {
                                    (&icons::VISIBLE, "Hide column")
                                } else {
                                    (&icons::INVISIBLE, "Show column")
                                };
                                if ui.small_icon_button(icon, alt_text).clicked() {
                                    TableColumn::save_visibility(
                                        blueprint_ctx,
                                        column.0.physical_name(),
                                        layout_kind,
                                        !visible,
                                    );
                                }
                            },
                        );
                    },
                );
                if response.is_drag_finished() {
                    response.update_vec(&mut columns);
                    TableBlueprint::save_column_order(
                        blueprint_ctx,
                        layout_kind,
                        columns.into_iter().map(|column| column.0),
                    );
                }
            });
        });
}
