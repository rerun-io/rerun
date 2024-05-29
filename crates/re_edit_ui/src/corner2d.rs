use re_types::blueprint::components::Corner2D;
use re_viewer_context::ViewerContext;

pub fn edit_corner2d(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut Corner2D,
) -> egui::Response {
    let outer_response = egui::ComboBox::from_id_source("corner2d")
        .selected_text(format!("{value}"))
        .show_ui(ui, |ui| {
            ui.selectable_value(
                value,
                egui_plot::Corner::LeftTop.into(),
                format!("{}", Corner2D::from(egui_plot::Corner::LeftTop)),
            )
            .union(ui.selectable_value(
                value,
                egui_plot::Corner::RightTop.into(),
                format!("{}", Corner2D::from(egui_plot::Corner::RightTop)),
            ))
            .union(ui.selectable_value(
                value,
                egui_plot::Corner::LeftBottom.into(),
                format!("{}", Corner2D::from(egui_plot::Corner::LeftBottom)),
            ))
            .union(ui.selectable_value(
                value,
                egui_plot::Corner::RightBottom.into(),
                format!("{}", Corner2D::from(egui_plot::Corner::RightBottom)),
            ))
        });

    outer_response.inner.unwrap_or(outer_response.response)
}
