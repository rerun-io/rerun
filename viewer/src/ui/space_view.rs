use std::ops::RangeInclusive;

use egui::{Rect, Vec2};

use data_store::ObjectsBySpace;
use itertools::Itertools;
use log_types::*;

use crate::{LogDb, Preview, Selection, ViewerContext};

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
enum SelectedSpace {
    All,
    /// None is the catch-all space for object without a space.
    Specific(Option<ObjPath>),
}

impl Default for SelectedSpace {
    fn default() -> Self {
        SelectedSpace::All
    }
}
// ----------------------------------------------------------------------------

#[derive(Clone)]
struct SpaceInfo {
    /// Path to the space.
    ///
    /// `None`: catch-all for all objects with no space assigned.
    space_path: Option<ObjPath>,

    /// Only set for 2D spaces
    size: Option<Vec2>,
}

impl SpaceInfo {
    fn obj_path_components(&self) -> Vec<ObjPathComp> {
        self.space_path
            .as_ref()
            .map(|space_path| ObjPathBuilder::from(space_path).as_slice().to_vec())
            .unwrap_or_default()
    }
}
// ----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct SpaceView {
    // per space
    state_2d: ahash::AHashMap<Option<ObjPath>, crate::view2d::State2D>,
    state_3d: ahash::AHashMap<Option<ObjPath>, crate::view3d::State3D>,

    selected: SelectedSpace,
}

impl SpaceView {
    pub fn ui(&mut self, log_db: &LogDb, context: &mut ViewerContext, ui: &mut egui::Ui) {
        crate::profile_function!();

        let objects = context
            .time_control
            .selected_objects(log_db)
            .partition_on_space();

        if false {
            use itertools::Itertools as _;
            ui.label(format!(
                "Spaces: {}",
                objects.keys().sorted().map(|s| space_name(*s)).format(" ")
            ));
        }

        if let Selection::Space(selected_space) = &context.selection {
            self.selected = SelectedSpace::Specific(Some(selected_space.clone()));
        }

        match self.selected.clone() {
            SelectedSpace::All => {
                self.show_all(log_db, &objects, context, ui);
            }
            SelectedSpace::Specific(selected_space) => {
                ui.horizontal(|ui| {
                    if ui.button("Show all spaces").clicked() {
                        self.selected = SelectedSpace::All;
                        if matches!(&context.selection, Selection::Space(_)) {
                            context.selection = Selection::None;
                        }
                    }
                });
                self.show_space(log_db, &objects, context, selected_space.as_ref(), ui);
            }
        }
    }

    fn show_all(
        &mut self,
        log_db: &LogDb,
        objects: &ObjectsBySpace<'_>,
        context: &mut ViewerContext,
        ui: &mut egui::Ui,
    ) {
        let space_infos = objects
            .keys()
            .sorted()
            .map(|opt_space_path| {
                let size = self
                    .state_2d
                    .get(&opt_space_path.cloned())
                    .and_then(|state| state.size());
                SpaceInfo {
                    space_path: opt_space_path.cloned(),
                    size,
                }
            })
            .collect_vec();

        let regions = layout_spaces(ui.available_rect_before_wrap(), &space_infos);

        for (rect, space_info) in itertools::izip!(&regions, &space_infos) {
            let mut ui = ui.child_ui_with_id_source(*rect, *ui.layout(), &space_info.space_path);
            egui::Frame::group(ui.style())
                .inner_margin(Vec2::splat(4.0))
                .show(&mut ui, |ui| {
                    ui.vertical_centered(|ui| {
                        if ui
                            .selectable_label(false, space_name(space_info.space_path.as_ref()))
                            .clicked()
                        {
                            self.selected = SelectedSpace::Specific(space_info.space_path.clone());
                        }
                        self.show_space(
                            log_db,
                            objects,
                            context,
                            space_info.space_path.as_ref(),
                            ui,
                        );
                        ui.allocate_space(ui.available_size());
                    });
                });
        }
    }

    fn show_space(
        &mut self,
        log_db: &LogDb,
        objects: &ObjectsBySpace<'_>,
        context: &mut ViewerContext,
        space: Option<&ObjPath>,
        ui: &mut egui::Ui,
    ) {
        crate::profile_function!(space_name(space));

        let objects = if let Some(objects) = objects.get(&space) {
            objects
        } else {
            return;
        };

        let objects = objects.filter(|props| {
            context
                .projected_object_properties
                .get(props.parent_obj_path)
                .visible
        });

        if objects.has_any_3d() {
            let state_3d = self.state_3d.entry(space.cloned()).or_default();
            crate::view3d::combined_view_3d(log_db, context, ui, state_3d, space, &objects);
        }

        if objects.has_any_2d() {
            let state_2d = self.state_2d.entry(space.cloned()).or_default();
            crate::view2d::combined_view_2d(log_db, context, ui, state_2d, &objects);
        }
    }
}

fn space_name(space: Option<&ObjPath>) -> String {
    if let Some(space) = space {
        space.to_string()
    } else {
        "<default space>".to_owned()
    }
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

fn group_by_path_prefix(space_infos: &[SpaceInfo]) -> Vec<Vec<SpaceInfo>> {
    if space_infos.len() < 2 {
        return vec![space_infos.to_vec()];
    }
    crate::profile_function!();

    let paths = space_infos
        .iter()
        .map(|space_info| space_info.obj_path_components())
        .collect_vec();

    for i in 0.. {
        let mut groups: std::collections::BTreeMap<Option<&ObjPathComp>, Vec<&SpaceInfo>> =
            Default::default();
        for (path, space) in paths.iter().zip(space_infos) {
            groups.entry(path.get(i)).or_default().push(space);
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
    space_infos
        .iter()
        .map(|space| vec![space.clone()])
        .collect()
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

// ----------------------------------------------------------------------------

pub(crate) fn show_log_msg(
    context: &mut ViewerContext,
    ui: &mut egui::Ui,
    msg: &DataMsg,
    preview: Preview,
) {
    let DataMsg {
        id,
        time_point,
        data_path,
        data,
    } = msg;

    egui::Grid::new("fields")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            ui.monospace("data_path:");
            ui.label(format!("{data_path}"));
            ui.end_row();

            ui.monospace("time_point:");
            ui_time_point(context, ui, time_point);
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
        Data::Batch { data, .. } => ui.label(format!("batch: {:?}", data)),

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

        Data::Vec2([x, y]) => ui.label(format!("[{x:.1}, {y:.1}]")),
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

        Data::Vec3([x, y, z]) => ui.label(format!("[{x:.3}, {y:.3}, {z:.3}]")),
        Data::Box3(_) => ui.label("3D box"),
        Data::Path3D(_) => ui.label("3D path"),
        Data::LineSegments3D(segments) => ui.label(format!("{} 3D line segments", segments.len())),
        Data::Mesh3D(_) => ui.label("3D mesh"),
        Data::Camera(_) => ui.label("Camera"),

        Data::Vecf32(data) => ui.label(format!("Vecf32({data:?})")),

        Data::Space(space) => {
            // ui.label(space.to_string())
            context.space_button(ui, space)
        }
    }
}
