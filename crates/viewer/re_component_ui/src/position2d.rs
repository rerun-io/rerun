use re_types::components::Position2D;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub fn singleline_edit_position2d(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, Position2D>,
) -> egui::Response {
    let x = value.0[0];
    let y = value.0[1];

    if let Some(value) = value.as_mut() {
        let mut x_edit = x;
        let mut y_edit = y;

        ui.label("[");
        let response_x = ui.add(egui::DragValue::new(&mut x_edit));
        ui.label(",");
        let response_y = ui.add(egui::DragValue::new(&mut y_edit));
        ui.label("]");
        let response = response_y | response_x;

        if response.changed() {
            *value = Position2D([x_edit, y_edit].into());
        }

        response
    } else {
        ui.label(format!(
            "[ {} , {} ]",
            re_format::format_f32(x),
            re_format::format_f32(y)
        ))
    }
}
