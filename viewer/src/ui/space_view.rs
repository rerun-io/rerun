use std::ops::RangeInclusive;

use ahash::AHashMap;

use egui::{Rect, Vec2};

use itertools::Itertools;
use log_types::*;

use crate::{LogDb, Preview, Selection, ViewerContext};

// ----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct SpaceView {
    // per space
    state_3d: AHashMap<ObjectPath, crate::view3d::State3D>,
}

impl SpaceView {
    pub fn ui(&mut self, log_db: &LogDb, context: &mut ViewerContext, ui: &mut egui::Ui) {
        crate::profile_function!();

        let messages = context.time_control.selected_messages(log_db);
        if messages.is_empty() {
            return;
        }

        // ui.small("Showing latest versions of each object.")
        //     .on_hover_text("Latest by the current time, that is");

        if let Selection::Space(selected_space) = &context.selection {
            let selected_space = selected_space.clone();
            ui.horizontal(|ui| {
                if ui.button("Show all spaces").clicked() {
                    context.selection = Selection::None;
                }
                context.space_button(ui, &selected_space);
            });
            self.show_space(log_db, &messages, context, &selected_space, ui);
        } else {
            self.show_all(log_db, &messages, context, ui);
        }
    }

    fn show_all(
        &mut self,
        log_db: &LogDb,
        messages: &[&LogMsg],
        context: &mut ViewerContext,
        ui: &mut egui::Ui,
    ) {
        let spaces = log_db
            .spaces
            .iter()
            .map(|(path, summary)| SpaceInfo {
                space_path: path.clone(),
                size: summary.size_2d(),
            })
            .collect_vec();

        let regions = layout_spaces(ui.available_rect_before_wrap(), &spaces);

        for (rect, space) in itertools::izip!(&regions, log_db.spaces.keys()) {
            let mut ui = ui.child_ui_with_id_source(*rect, *ui.layout(), space);
            egui::Frame::group(ui.style())
                .inner_margin(Vec2::splat(4.0))
                .show(&mut ui, |ui| {
                    ui.vertical_centered(|ui| {
                        context.space_button(ui, space);
                        self.show_space(log_db, messages, context, &space.clone(), ui);
                        ui.allocate_space(ui.available_size());
                    });
                });
        }
    }
}

#[derive(Clone)]
struct SpaceInfo {
    /// Path to the space
    space_path: ObjectPath,

    /// Only set for 2D spaces
    size: Option<Vec2>,
}

fn layout_spaces(available_rect: Rect, spaces: &[SpaceInfo]) -> Vec<Rect> {
    if spaces.is_empty() {
        return vec![];
    } else if spaces.len() == 1 {
        return vec![available_rect];
    }

    let desired_aspect_ratio = desired_aspect_ratio(spaces).unwrap_or(16.0 / 9.0);

    let groups = group_by_path_prefix(spaces);
    assert!(groups.len() > 1);

    // TODO: if there are a lot of groups (>3) we likely want to put them in a grid instead of doing a linear split (like we do below)

    if available_rect.width() > desired_aspect_ratio * available_rect.height() {
        // left-to-right
        let x_ranges = weighted_split(available_rect.x_range(), &groups);
        x_ranges
            .iter()
            .cloned()
            .zip(&groups)
            .flat_map(|(x_range, group)| {
                let sub_rect = Rect::from_x_y_ranges(x_range, available_rect.y_range());
                layout_spaces(sub_rect, group)
            })
            .collect()
    } else {
        // top-to-bottom
        let y_ranges = weighted_split(available_rect.y_range(), &groups);
        y_ranges
            .iter()
            .cloned()
            .zip(&groups)
            .flat_map(|(y_range, group)| {
                let sub_rect = Rect::from_x_y_ranges(available_rect.x_range(), y_range);
                layout_spaces(sub_rect, group)
            })
            .collect()
    }
}

fn desired_aspect_ratio(spaces: &[SpaceInfo]) -> Option<f32> {
    let mut sum = 0.0;
    let mut num = 0.0;
    for space in spaces {
        if let Some(size) = space.size {
            let aspect = size.x / size.y;
            if aspect.is_finite() {
                sum += aspect;
                num += 1.0;
            }
        }
    }

    if num == 0.0 {
        None
    } else {
        Some(sum / num)
    }
}

fn group_by_path_prefix(spaces: &[SpaceInfo]) -> Vec<Vec<SpaceInfo>> {
    if spaces.len() < 2 {
        return vec![spaces.to_vec()];
    }

    for i in 0.. {
        let mut groups: std::collections::BTreeMap<Option<&ObjectPathComponent>, Vec<&SpaceInfo>> =
            Default::default();
        for space in spaces {
            groups
                .entry(space.space_path.0.get(i))
                .or_default()
                .push(space);
        }
        if groups.len() == 1 && groups.contains_key(&None) {
            break;
        }
        if groups.len() > 1 {
            return groups
                .values()
                .map(|spaces| spaces.iter().cloned().cloned().collect())
                .collect();
        }
    }
    spaces.iter().map(|space| vec![space.clone()]).collect()
}

fn weighted_split(
    range: RangeInclusive<f32>,
    groups: &[Vec<SpaceInfo>],
) -> Vec<RangeInclusive<f32>> {
    let weights: Vec<f64> = groups
        .iter()
        .map(|group| (group.len() as f64).sqrt())
        .collect();
    let total_weight: f64 = weights.iter().sum();

    let mut w_accum: f64 = 0.0;
    weights
        .iter()
        .map(|&w| {
            let l = egui::lerp(range.clone(), (w_accum / total_weight) as f32);
            w_accum += w;
            let r = egui::lerp(range.clone(), (w_accum / total_weight) as f32);
            l..=r
        })
        .collect()
}

impl SpaceView {
    fn show_space(
        &mut self,
        log_db: &LogDb,
        messages: &[&LogMsg],
        context: &mut ViewerContext,
        space: &ObjectPath,
        ui: &mut egui::Ui,
    ) {
        crate::profile_function!(&space.to_string());

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

        if !space_summary.messages_3d.is_empty() {
            let state_3d = self.state_3d.entry(space.clone()).or_default();
            crate::view3d::combined_view_3d(
                log_db,
                context,
                ui,
                state_3d,
                space,
                space_summary,
                messages,
            );
        }

        if !space_summary.messages_2d.is_empty() {
            crate::view2d::combined_view_2d(log_db, context, ui, space, space_summary, messages);
        }
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
        Data::Color([r, g, b, a]) => {
            let color = egui::Color32::from_rgba_unmultiplied(*r, *g, *b, *a);
            let response = egui::color_picker::show_color(ui, color, Vec2::new(32.0, 16.0));
            ui.painter().rect_stroke(
                response.rect,
                1.0,
                ui.visuals().widgets.noninteractive.fg_stroke,
            );
            response.on_hover_text(format!("Color #{:02x}{:02x}{:02x}{:02x}", r, g, b, a))
        }

        Data::Pos2([x, y]) => ui.label(format!("Pos2({x:.1}, {y:.1})")),
        Data::LineSegments2D(linesegments) => {
            ui.label(format!("{} 2D line segment(s)", linesegments.len()))
        }
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

        Data::Pos3([x, y, z]) => ui.label(format!("Pos3({x:.3}, {y:.3}, {z:.3})")),
        Data::Vec3([x, y, z]) => ui.label(format!("Vec3({x:.3}, {y:.3}, {z:.3})")),
        Data::Box3(_) => ui.label("3D box"),
        Data::Path3D(_) => ui.label("3D path"),
        Data::LineSegments3D(segments) => ui.label(format!("{} 3D line segments", segments.len())),
        Data::Mesh3D(_) => ui.label("3D mesh"),
        Data::Camera(_) => ui.label("Camera"),

        Data::Vecf32(data) => ui.label(format!("Vecf32({data:?})")),
    }
}
