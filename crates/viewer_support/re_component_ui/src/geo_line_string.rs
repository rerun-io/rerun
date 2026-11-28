use re_format::{format_lat_lon, format_uint};
use re_sdk_types::components::GeoLineString;
use re_ui::{TableStyle, UiExt as _, embedded_table_ui};
use re_viewer_context::{AppContext, MaybeMutRef, UiLayout};

fn singleline_view_geo_line_string(
    _ctx: &AppContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, GeoLineString>,
) -> egui::Response {
    UiLayout::List.label(
        ui,
        format!("{} positions", format_uint(value.as_ref().0.len())),
    )
}

fn multiline_view_geo_line_string(
    _ctx: &AppContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, GeoLineString>,
) -> egui::Response {
    // TODO(andreas): Editing this would be nice!
    let positions = &value.as_ref().0;

    embedded_table_ui(
        ui,
        "geo_line_string",
        TableStyle::Dense,
        &[("Latitude", 100.0), ("Longitude", 100.0)],
        positions.len(),
        |ui, row, column| {
            let Some(pos) = positions.get(row) else {
                return;
            };
            let (coordinate, help) = match column {
                0 => (
                    pos.x(),
                    "Latitude according to [EPSG:4326](https://epsg.io/4326)",
                ),
                1 => (
                    pos.y(),
                    "Longitude according to [EPSG:4326](https://epsg.io/4326)",
                ),
                _ => return,
            };
            ui.label(format_lat_lon(coordinate)).on_hover_ui(|ui| {
                ui.markdown_ui(help);
            });
        },
    );

    // Placeholder response.
    ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
}

pub fn register_geo_line_string_component_ui(
    registry: &mut re_viewer_context::ComponentUiRegistry,
) {
    registry.add_multiline_edit_or_view(multiline_view_geo_line_string);
    registry.add_singleline_edit_or_view(singleline_view_geo_line_string);
}
