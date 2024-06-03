use egui::NumExt as _;
use re_types::blueprint::components::VisualBounds2D;
use re_viewer_context::ViewerContext;

pub fn edit_visual_bounds_2d(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut VisualBounds2D,
) -> egui::Response {
    let speed_func = |start: f64, end: f64| ((end - start).abs() * 0.01).at_least(0.001);

    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            let [x_range_start, x_range_end] = &mut value.x_range.0;
            let speed = speed_func(*x_range_start, *x_range_end);

            ui.label("x");
            ui.add(
                egui::DragValue::new(x_range_start)
                    .clamp_range(f64::NEG_INFINITY..=*x_range_end)
                    .max_decimals(2)
                    .speed(speed),
            ) | ui.add(
                egui::DragValue::new(x_range_end)
                    .clamp_range(*x_range_start..=f64::INFINITY)
                    .max_decimals(2)
                    .speed(speed),
            )
        })
        .inner
            | ui.horizontal(|ui| {
                let [y_range_start, y_range_end] = &mut value.y_range.0;
                let speed = speed_func(*y_range_start, *y_range_end);

                ui.label("y");
                ui.add(
                    egui::DragValue::new(y_range_start)
                        .clamp_range(f64::NEG_INFINITY..=*y_range_end)
                        .max_decimals(2)
                        .speed(speed),
                ) | ui.add(
                    egui::DragValue::new(y_range_end)
                        .clamp_range(*y_range_start..=f64::INFINITY)
                        .max_decimals(2)
                        .speed(speed),
                )
            })
            .inner
    })
    .inner
}
