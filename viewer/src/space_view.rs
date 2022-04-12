use eframe::egui;
use egui::{NumExt as _, Rect, Vec2};

use log_types::*;

use crate::{
    viewer_context::{Selection, ViewerContext},
    LogDb, Preview,
};

// ----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct SpaceView {
    state_3d: crate::view_3d::State3D,
}

impl SpaceView {
    pub fn ui(&mut self, log_db: &LogDb, context: &mut ViewerContext, ui: &mut egui::Ui) {
        ui.small("Showing latest versions of each object.")
            .on_hover_text("Latest by the current time, that is");

        if let Selection::Space(selected_space) = &context.selection {
            let selected_space = selected_space.clone();
            ui.horizontal(|ui| {
                if ui.button("Show all spaces").clicked() {
                    context.selection = Selection::None;
                }
                context.space_button(ui, &selected_space);
            });
            self.show_space(log_db, context, &selected_space, ui);
        } else {
            self.show_all(log_db, context, ui);
        }
    }

    fn show_all(&mut self, log_db: &LogDb, context: &mut ViewerContext, ui: &mut egui::Ui) {
        let regions = gridify(ui.available_rect_before_wrap(), log_db.spaces.len());
        for (rect, space) in itertools::izip!(&regions, log_db.spaces.keys()) {
            let mut ui = ui.child_ui_with_id_source(*rect, *ui.layout(), space);
            egui::Frame::group(ui.style())
                .outer_margin(Vec2::splat(4.0))
                .show(&mut ui, |ui| {
                    ui.vertical_centered(|ui| {
                        context.space_button(ui, space);
                        self.show_space(log_db, context, &space.clone(), ui);
                        ui.allocate_space(ui.available_size());
                    });
                });
        }
    }
}

fn gridify(available_rect: Rect, num_cells: usize) -> Vec<Rect> {
    if num_cells == 0 {
        return vec![];
    }

    // let desired_aspect_ratio = 4.0/3.0; // TODO

    // TODO: a smart algorithm for choosing the number of rows
    let num_rows = 2;

    let mut rects = Vec::with_capacity(num_cells);

    let mut cells_left = num_cells;

    for row in 0..num_rows {
        let top = egui::lerp(available_rect.y_range(), row as f32 / num_rows as f32);
        let bottom = egui::lerp(available_rect.y_range(), (row + 1) as f32 / num_rows as f32);

        let cols_in_row = if row < num_rows - 1 {
            ((num_cells as f32 / num_rows as f32).ceil() as usize).at_most(cells_left)
        } else {
            cells_left
        };

        for col in 0..cols_in_row {
            let left = egui::lerp(available_rect.x_range(), col as f32 / cols_in_row as f32);
            let right = egui::lerp(
                available_rect.x_range(),
                (col + 1) as f32 / cols_in_row as f32,
            );
            rects.push(egui::Rect {
                min: egui::pos2(left, top),
                max: egui::pos2(right, bottom),
            });
        }

        cells_left -= cols_in_row;
    }
    assert_eq!(cells_left, 0);
    assert_eq!(rects.len(), num_cells);
    rects
}

impl SpaceView {
    fn show_space(
        &mut self,
        log_db: &LogDb,
        context: &mut ViewerContext,
        space: &ObjectPath,
        ui: &mut egui::Ui,
    ) {
        let space_summary = if let Some(space_summary) = log_db.spaces.get(space) {
            space_summary
        } else {
            ui.label("[missing space]");
            return;
        };

        // ui.label(format!(
        //     "{} log lines in this space",
        //     space_summary.messages.len()
        // ));

        let latest = context.time_control.latest_of_each_object_vec(log_db);
        if latest.is_empty() {
            return;
        }

        crate::view_3d::combined_view_3d(
            log_db,
            context,
            ui,
            &mut self.state_3d,
            space,
            space_summary,
            &latest,
        );

        crate::view_2d::combined_view_2d(log_db, context, ui, space, space_summary, latest);
    }
}

// ----------------------------------------------------------------------------

pub(crate) fn show_log_msg(
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    msg: &LogMsg,
    preview: Preview,
) {
    let LogMsg {
        id,
        time_point,
        object_path,
        space,
        data,
    } = msg;

    egui::Grid::new("fields")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            ui.monospace("object_path:");
            ui.label(format!("{object_path}"));
            ui.end_row();

            ui.monospace("time_point:");
            ui_time_point(context, ui, time_point);
            ui.end_row();

            ui.monospace("space:");
            if let Some(space) = space {
                context.space_button(ui, space);
            }
            ui.end_row();

            ui.monospace("data:");
            ui_data(context, ui, id, data, preview);
            ui.end_row();
        });
}

pub(crate) fn ui_time_point(
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    time_point: &TimePoint,
) {
    ui.vertical(|ui| {
        egui::Grid::new("time_point").num_columns(2).show(ui, |ui| {
            for (time_source, value) in &time_point.0 {
                ui.label(format!("{time_source}:"));
                context.time_button(ui, time_source, *value);
                ui.end_row();
            }
        });
    });
}

pub(crate) fn ui_data(
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    id: &LogId,
    data: &Data,
    preview: Preview,
) -> egui::Response {
    match data {
        Data::I32(value) => ui.label(value.to_string()),
        Data::F32(value) => ui.label(value.to_string()),
        Data::Color([r, g, b, a]) => ui.label(format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a)),
        Data::Pos2([x, y]) => ui.label(format!("Pos2({x:.1}, {y:.1})")),
        Data::BBox2D(bbox) => ui.label(format!(
            "BBox2D(min: [{:.1} {:.1}], max: [{:.1} {:.1}])",
            bbox.min[0], bbox.min[1], bbox.max[0], bbox.max[1]
        )),
        Data::Image(image) => {
            let egui_image = context.image_cache.get(id, image);
            ui.horizontal_centered(|ui| {
                let max_width = match preview {
                    Preview::Small => 32.0,
                    Preview::Medium => 128.0,
                    Preview::Specific(height) => height,
                };

                egui_image
                    .show_max_size(ui, Vec2::new(4.0 * max_width, max_width))
                    .on_hover_ui(|ui| {
                        egui_image.show(ui);
                    });

                ui.label(format!("{}x{}", image.size[0], image.size[1]));
            })
            .response
        }
        Data::Pos3([x, y, z]) => ui.label(format!("Pos3({x}, {y}, {z})")),
        Data::LineSegments3D(segments) => ui.label(format!("{} 3D line segments", segments.len())),
        Data::Mesh3D(_) => ui.label("3D mesh"),
        Data::Vecf32(data) => ui.label(format!("Vecf32({data:?})")),
    }
}
