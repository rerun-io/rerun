use egui::NumExt as _;

use re_types::components::Range1D;

pub fn edit_range1d(
    _ctx: &re_viewer_context::ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut Range1D,
) -> egui::Response {
    let [min, max] = &mut value.0 .0;
    let range = (*max - *min).abs();
    let speed = (range * 0.01).at_least(0.001);

    ui.label("Min:");
    let response_min = ui.add(
        egui::DragValue::new(min)
            .clamp_range(f64::NEG_INFINITY..=*max)
            .speed(speed),
    );
    ui.label("Max:");
    let response_max = ui.add(
        egui::DragValue::new(max)
            .clamp_range(*min..=f64::INFINITY)
            .speed(speed),
    );

    response_min.union(response_max)
}
