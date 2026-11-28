use re_format::{format_f32, format_uint};
use re_sdk_types::components::{LineStrip2D, LineStrip3D};
use re_ui::{TableStyle, embedded_table_ui};
use re_viewer_context::{AppContext, MaybeMutRef, UiLayout};

use crate::DEFAULT_NUMBER_WIDTH;

fn singleline_view_line_strip_3d(
    _ctx: &AppContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, LineStrip3D>,
) -> egui::Response {
    UiLayout::List.label(
        ui,
        format!("{} positions", format_uint(value.as_ref().0.len())),
    )
}

fn multiline_view_line_strip_3d(
    _ctx: &AppContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, LineStrip3D>,
) -> egui::Response {
    // TODO(andreas): Editing this would be nice!
    let positions = &value.as_ref().0;

    embedded_table_ui(
        ui,
        "line_strip_3d",
        TableStyle::Dense,
        &[
            ("x", DEFAULT_NUMBER_WIDTH),
            ("y", DEFAULT_NUMBER_WIDTH),
            ("z", DEFAULT_NUMBER_WIDTH),
        ],
        positions.len(),
        |ui, row, column| {
            if let Some(pos) = positions.get(row)
                && let Some(&coordinate) = [pos.x(), pos.y(), pos.z()].get(column)
            {
                ui.label(format_f32(coordinate));
            }
        },
    );

    // Placeholder response.
    ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
}

fn singleline_view_line_strip_2d(
    _ctx: &AppContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, LineStrip2D>,
) -> egui::Response {
    UiLayout::List.label(
        ui,
        format!("{} positions", format_uint(value.as_ref().0.len())),
    )
}

fn multiline_view_line_strip_2d(
    _ctx: &AppContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, LineStrip2D>,
) -> egui::Response {
    // TODO(andreas): Editing this would be nice!
    let positions = &value.as_ref().0;

    embedded_table_ui(
        ui,
        "line_strip_2d",
        TableStyle::Dense,
        &[("x", DEFAULT_NUMBER_WIDTH), ("y", DEFAULT_NUMBER_WIDTH)],
        positions.len(),
        |ui, row, column| {
            if let Some(pos) = positions.get(row)
                && let Some(&coordinate) = [pos.x(), pos.y()].get(column)
            {
                ui.label(format_f32(coordinate));
            }
        },
    );

    // Placeholder response.
    ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
}

pub fn register_linestrip_component_ui(registry: &mut re_viewer_context::ComponentUiRegistry) {
    registry.add_multiline_edit_or_view(multiline_view_line_strip_3d);
    registry.add_singleline_edit_or_view(singleline_view_line_strip_3d);
    registry.add_multiline_edit_or_view(multiline_view_line_strip_2d);
    registry.add_singleline_edit_or_view(singleline_view_line_strip_2d);
}
