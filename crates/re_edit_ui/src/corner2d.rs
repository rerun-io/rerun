use re_types::blueprint::components::Corner2D;
use re_viewer_context::ViewerContext;

use crate::response_utils::response_with_changes_of_inner;

pub fn edit_corner2d(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut Corner2D,
) -> egui::Response {
    response_with_changes_of_inner(
        egui::ComboBox::from_id_source("corner2d")
            .selected_text(format!("{value}"))
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    value,
                    egui_plot::Corner::LeftTop.into(),
                    format!("{}", Corner2D::from(egui_plot::Corner::LeftTop)),
                ) | ui.selectable_value(
                    value,
                    egui_plot::Corner::RightTop.into(),
                    format!("{}", Corner2D::from(egui_plot::Corner::RightTop)),
                ) | ui.selectable_value(
                    value,
                    egui_plot::Corner::LeftBottom.into(),
                    format!("{}", Corner2D::from(egui_plot::Corner::LeftBottom)),
                ) | ui.selectable_value(
                    value,
                    egui_plot::Corner::RightBottom.into(),
                    format!("{}", Corner2D::from(egui_plot::Corner::RightBottom)),
                )
            }),
    )
}
