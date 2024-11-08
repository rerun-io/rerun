use re_format::{format_lat_lon, format_uint};
use re_types::components::GeoLineString;
use re_ui::UiExt as _;
use re_viewer_context::{MaybeMutRef, UiLayout, ViewerContext};

fn singleline_view_geo_line_string(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, GeoLineString>,
) -> egui::Response {
    UiLayout::List.label(
        ui,
        format!("{} positions", format_uint(value.as_ref().0.len())),
    )
}

fn multiline_view_geo_line_string(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, GeoLineString>,
) -> egui::Response {
    use egui_extras::Column;

    // TODO(andreas): Editing this would be nice!
    let value = value.as_ref();

    UiLayout::SelectionPanelFull
        .table(ui)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .columns(Column::initial(100.0).clip(true), 2)
        .header(re_ui::DesignTokens::table_header_height(), |mut header| {
            re_ui::DesignTokens::setup_table_header(&mut header);
            header.col(|ui| {
                ui.label("Latitude");
            });
            header.col(|ui| {
                ui.label("Longitude");
            });
        })
        .body(|mut body| {
            re_ui::DesignTokens::setup_table_body(&mut body);
            let row_height = re_ui::DesignTokens::table_line_height();
            body.rows(row_height, value.0.len(), |mut row| {
                if let Some(pos) = value.0.get(row.index()) {
                    row.col(|ui| {
                        ui.label(format_lat_lon(pos.x())).on_hover_ui(|ui| {
                            ui.markdown_ui(
                                "Latitude according to [EPSG:4326](https://epsg.io/4326)",
                            );
                        });
                    });
                    row.col(|ui| {
                        ui.label(format_lat_lon(pos.y())).on_hover_ui(|ui| {
                            ui.markdown_ui(
                                "Longitude according to [EPSG:4326](https://epsg.io/4326)",
                            );
                        });
                    });
                }
            });
        });

    // Placeholder response.
    ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover())
}

pub fn register_geo_line_string_component_ui(
    registry: &mut re_viewer_context::ComponentUiRegistry,
) {
    registry.add_multiline_edit_or_view(multiline_view_geo_line_string);
    registry.add_singleline_edit_or_view(singleline_view_geo_line_string);
}
