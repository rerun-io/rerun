use re_format::format_lat_lon;
use re_sdk_types::components::LatLon;
use re_ui::UiExt as _;
use re_viewer_context::{MaybeMutRef, UiLayout, ViewerContext};

pub fn singleline_view_lat_lon(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, LatLon>,
) -> egui::Response {
    let value = value.as_ref();
    UiLayout::List
        .label(
            ui,
            format!(
                "{}, {}",
                format_lat_lon(value.latitude()),
                format_lat_lon(value.longitude())
            ),
        )
        .on_hover_ui(|ui| {
            ui.markdown_ui("Latitude and longitude according to [EPSG:4326](https://epsg.io/4326)");
        })
}
