use egui::NumExt as _;
use re_sdk_types::blueprint::components::VisualBounds2D;
use re_sdk_types::datatypes::Range2D;
use re_ui::UiExt as _;
use re_viewer_context::{MaybeMutRef, ViewerContext};

pub fn multiline_edit_visual_bounds2d(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, VisualBounds2D>,
) -> egui::Response {
    let mut any_edit = false;

    let response_x = ui.list_item().interactive(false).show_hierarchical(
        ui,
        re_ui::list_item::PropertyContent::new("x").value_fn(|ui, _| {
            if let Some(value) = value.as_mut() {
                any_edit |= range_mut_ui(ui, &mut value.x_range.0).changed();
            } else {
                range_ui(ui, value.x_range.0);
            }
        }),
    );

    let response_y = ui.list_item().interactive(false).show_hierarchical(
        ui,
        re_ui::list_item::PropertyContent::new("y").value_fn(|ui, _| {
            if let Some(value) = value.as_mut() {
                any_edit |= range_mut_ui(ui, &mut value.y_range.0).changed();
            } else {
                range_ui(ui, value.y_range.0);
            }
        }),
    );

    let mut response = response_x | response_y;
    if any_edit {
        response.mark_changed();
    }
    response
}

fn range_ui(ui: &mut egui::Ui, [start, end]: [f64; 2]) {
    ui.horizontal_centered(|ui| {
        ui.label(format!(
            "{} - {}",
            re_format::format_f64(start),
            re_format::format_f64(end)
        ));
    });
}

fn range_mut_ui(ui: &mut egui::Ui, [start, end]: &mut [f64; 2]) -> egui::Response {
    let speed_func = |start: f64, end: f64| ((end - start).abs() * 0.01).at_least(0.001);

    let speed = speed_func(*start, *end);

    ui.horizontal_centered(|ui| {
        let response_min = ui.add(
            egui::DragValue::new(start)
                .clamp_existing_to_range(false)
                .range(f64::MIN..=*end)
                .max_decimals(2)
                .speed(speed),
        );

        ui.label("-");

        let response_max = ui.add(
            egui::DragValue::new(end)
                .clamp_existing_to_range(false)
                .range(*start..=f64::MAX)
                .max_decimals(2)
                .speed(speed),
        );

        response_min | response_max
    })
    .inner
}

pub fn singleline_edit_visual_bounds2d(
    _ctx: &ViewerContext<'_>,
    ui: &mut egui::Ui,
    value: &mut MaybeMutRef<'_, VisualBounds2D>,
) -> egui::Response {
    if let Some(value) = value.as_mut() {
        // Not a lot of space in a single line, so edit width/height instead.
        let width = value.x_range.0[1] - value.x_range.0[0];
        let height = value.y_range.0[1] - value.y_range.0[0];

        let mut width_edit = width;
        let mut height_edit = height;

        let speed_func = |v: f64| (v.abs() * 0.01).at_least(0.001);

        let response_width = ui.add(
            egui::DragValue::new(&mut width_edit)
                .clamp_existing_to_range(false)
                .range(0.001..=f64::MAX)
                .max_decimals(1)
                .speed(speed_func(width)),
        );
        ui.label("×");
        let response_height = ui.add(
            egui::DragValue::new(&mut height_edit)
                .clamp_existing_to_range(false)
                .range(0.001..=f64::MAX)
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
    } else {
        let width = value.x_range.0[1] - value.x_range.0[0];
        let height = value.y_range.0[1] - value.y_range.0[0];
        ui.label(format!(
            "{} × {}",
            re_format::format_f64(width),
            re_format::format_f64(height)
        ))
    }
}
