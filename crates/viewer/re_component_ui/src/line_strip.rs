use re_format::{format_f32, format_uint};
use re_types::components::{LineStrip2D, LineStrip3D};
use re_viewer_context::{MaybeMutRef, UiLayout, ViewerContext};

use crate::DEFAULT_NUMBER_WIDTH;

fn singleline_view_line_strip_3d(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, LineStrip3D>,
) -> egui::Response {
    UiLayout::List.label(
        ui,
        format!("{} positions", format_uint(value.as_ref().0.len())),
    )
}

fn multiline_view_line_strip_3d(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, LineStrip3D>,
) -> egui::Response {
    use egui_extras::Column;

    // TODO(andreas): Editing this would be nice!
    let value = value.as_ref();

    // TODO(andreas): Is it really a good idea to always have the full table here?
    // Can we use the ui stack to know where we are and do the right thing instead?
    UiLayout::SelectionPanelFull
        .table(ui)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .columns(Column::initial(DEFAULT_NUMBER_WIDTH).clip(true), 3)
        .header(re_ui::DesignTokens::table_header_height(), |mut header| {
            re_ui::DesignTokens::setup_table_header(&mut header);
            header.col(|ui| {
                ui.label("x");
            });
            header.col(|ui| {
                ui.label("y");
            });
            header.col(|ui| {
                ui.label("z");
            });
        })
        .body(|mut body| {
            re_ui::DesignTokens::setup_table_body(&mut body);
            let row_height = re_ui::DesignTokens::table_line_height();
            body.rows(row_height, value.0.len(), |mut row| {
                if let Some(pos) = value.0.get(row.index()) {
                    row.col(|ui| {
                        ui.label(format_f32(pos.x()));
                    });
                    row.col(|ui| {
                        ui.label(format_f32(pos.y()));
                    });
                    row.col(|ui| {
                        ui.label(format_f32(pos.z()));
                    });
                }
            });
        });

    // Placeholder response.
    ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
}

fn singleline_view_line_strip_2d(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, LineStrip2D>,
) -> egui::Response {
    UiLayout::List.label(
        ui,
        format!("{} positions", format_uint(value.as_ref().0.len())),
    )
}

fn multiline_view_line_strip_2d(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, LineStrip2D>,
) -> egui::Response {
    use egui_extras::Column;

    // TODO(andreas): Editing this would be nice!
    let value = value.as_ref();

    // TODO(andreas): Is it really a good idea to always have the full table here?
    // Can we use the ui stack to know where we are and do the right thing instead?
    UiLayout::SelectionPanelFull
        .table(ui)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .columns(Column::initial(DEFAULT_NUMBER_WIDTH).clip(true), 2)
        .header(re_ui::DesignTokens::table_header_height(), |mut header| {
            re_ui::DesignTokens::setup_table_header(&mut header);
            header.col(|ui| {
                ui.label("x");
            });
            header.col(|ui| {
                ui.label("y");
            });
        })
        .body(|mut body| {
            re_ui::DesignTokens::setup_table_body(&mut body);
            let row_height = re_ui::DesignTokens::table_line_height();
            body.rows(row_height, value.0.len(), |mut row| {
                if let Some(pos) = value.0.get(row.index()) {
                    row.col(|ui| {
                        ui.label(format_f32(pos.x()));
                    });
                    row.col(|ui| {
                        ui.label(format_f32(pos.y()));
                    });
                }
            });
        });

    // Placeholder response.
    ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
}

pub fn register_linestrip_component_ui(registry: &mut re_viewer_context::ComponentUiRegistry) {
    registry.add_multiline_edit_or_view(multiline_view_line_strip_3d);
    registry.add_singleline_edit_or_view(singleline_view_line_strip_3d);
    registry.add_multiline_edit_or_view(multiline_view_line_strip_2d);
    registry.add_singleline_edit_or_view(singleline_view_line_strip_2d);
}
