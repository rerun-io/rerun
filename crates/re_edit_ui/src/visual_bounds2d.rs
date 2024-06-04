use egui::NumExt as _;
use re_types::{blueprint::components::VisualBounds2D, datatypes::Range2D};
use re_viewer_context::ViewerContext;

pub fn multiline_edit_visual_bounds2d(
    ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut VisualBounds2D,
) -> egui::Response {
    let speed_func = |start: f64, end: f64| ((end - start).abs() * 0.01).at_least(0.001);

    let mut any_edit = false;

    let response_x = re_ui::list_item::ListItem::new(ctx.re_ui)
        .interactive(false)
        .show_hierarchical(
            ui,
            re_ui::list_item::PropertyContent::new("x").value_fn(|_, ui, _| {
                let [x_range_start, x_range_end] = &mut value.x_range.0;
                let speed = speed_func(*x_range_start, *x_range_end);

                let response = ui
                    .horizontal_centered(|ui| {
                        let response_min = ui.add(
                            egui::DragValue::new(x_range_start)
                                .clamp_range(f64::MIN..=*x_range_end)
                                .max_decimals(2)
                                .speed(speed),
                        );

                        ui.label("-");

                        let response_max = ui.add(
                            egui::DragValue::new(x_range_end)
                                .clamp_range(*x_range_start..=f64::MAX)
                                .max_decimals(2)
                                .speed(speed),
                        );

                        response_min | response_max
                    })
                    .inner;

                if response.changed() {
                    any_edit = true;
                }
            }),
        );

    let response_y = re_ui::list_item::ListItem::new(ctx.re_ui)
        .interactive(false)
        .show_hierarchical(
            ui,
            re_ui::list_item::PropertyContent::new("y").value_fn(|_, ui, _| {
                let [y_range_start, y_range_end] = &mut value.y_range.0;
                let speed = speed_func(*y_range_start, *y_range_end);

                let response = ui
                    .horizontal_centered(|ui| {
                        let response_min = ui.add(
                            egui::DragValue::new(y_range_start)
                                .clamp_range(f64::MIN..=*y_range_end)
                                .max_decimals(2)
                                .speed(speed),
                        );

                        ui.label("-");

                        let response_max = ui.add(
                            egui::DragValue::new(y_range_end)
                                .clamp_range(*y_range_start..=f64::MAX)
                                .max_decimals(2)
                                .speed(speed),
                        );

                        response_min | response_max
                    })
                    .inner;

                if response.changed() {
                    any_edit = true;
                }
            }),
        );

    let mut response = response_x | response_y;
    if any_edit {
        response.mark_changed();
    }
    response
}

pub fn singleline_edit_visual_bounds2d(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut VisualBounds2D,
) -> egui::Response {
    // Not a lot of space in a single line, so edit width/height instead.
    let width = value.x_range.0[1] - value.x_range.0[0];
    let height = value.y_range.0[1] - value.y_range.0[0];

    let mut width_edit = width;
    let mut height_edit = height;

    let speed_func = |v: f64| (v.abs() * 0.01).at_least(0.001);

    let response_width = ui.add(
        egui::DragValue::new(&mut width_edit)
            .clamp_range(0.001..=f64::MAX)
            .max_decimals(1)
            .speed(speed_func(width)),
    );
    ui.label("×");
    let response_height = ui.add(
        egui::DragValue::new(&mut height_edit)
            .clamp_range(0.001..=f64::MAX)
            .max_decimals(1)
            .speed(speed_func(height)),
    );
    let response = response_height | response_width;

    // Empirically it's quite confusing to edit width/height separately for the visual bounds.
    // So we lock the aspect ratio.

    if response.changed() {
        let aspect_ratio = width / height;

        if width != width_edit {
            height_edit = width_edit / aspect_ratio;
        } else {
            width_edit = height_edit * aspect_ratio;
        }

        let d_width = width_edit - width;
        let d_height = height_edit - height;

        *value = Range2D {
            x_range: [
                value.x_range.0[0] - d_width * 0.5,
                value.x_range.0[1] + d_width * 0.5,
            ]
            .into(),
            y_range: [
                value.y_range.0[0] - d_height * 0.5,
                value.y_range.0[1] + d_height * 0.5,
            ]
            .into(),
        }
        .into();
    }

    response
}
