use std::ops::RangeInclusive;

use egui::{Rect, Vec2};
use itertools::Itertools as _;

use re_data_store::ObjectsBySpace;
use re_log_types::*;

use crate::{Preview, Selection, ViewerContext};

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
    state_2d: ahash::HashMap<Option<ObjPath>, crate::view2d::State2D>,
    state_3d: ahash::HashMap<Option<ObjPath>, crate::view3d::State3D>,

    selected: SelectedSpace,
}

impl SpaceView {
    pub fn ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        crate::profile_function!();

        if ctx.log_db.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.heading("No data");
            });
            return;
        }

        let objects = ctx
            .rec_cfg
            .time_ctrl
            .selected_objects(ctx.log_db)
            .partition_on_space();

        // `objects` contain all spaces that exist in this time,
        // but we want to show all spaces that could ever exist.
        // Othewise we get a lot of flicker of spaces as we play back data.
        let mut all_spaces = ctx.log_db.spaces().map(Some).collect_vec();
        if objects.contains_key(&None) {
            // Some objects lack a space, so they end up in the `None` space.
            // TODO(emilk): figure this out beforehand somehow.
            all_spaces.push(None);
        }
        all_spaces.sort_unstable();
        let all_spaces = all_spaces;

        if false {
            ui.label(format!(
                "Spaces: {}",
                all_spaces.iter().map(|&s| space_name(s)).format(" ")
            ));
        }

        if let Selection::Space(selected_space) = &ctx.rec_cfg.selection {
            self.selected = SelectedSpace::Specific(Some(selected_space.clone()));
        }

        match self.selected.clone() {
            SelectedSpace::All => {
                self.show_all(ctx, &all_spaces, &objects, ui);
            }
            SelectedSpace::Specific(selected_space) => {
                ui.horizontal(|ui| {
                    if ui.button("Show all spaces").clicked() {
                        self.selected = SelectedSpace::All;
                        if matches!(&ctx.rec_cfg.selection, Selection::Space(_)) {
                            ctx.rec_cfg.selection = Selection::None;
                        }
                    }
                });
                self.show_space(ctx, &objects, selected_space.as_ref(), ui);
            }
        }
    }

    fn show_all(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        all_spaces: &[Option<&ObjPath>],
        objects: &ObjectsBySpace<'_>,
        ui: &mut egui::Ui,
    ) {
        let space_infos = all_spaces
            .iter()
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
                        self.show_space(ctx, objects, space_info.space_path.as_ref(), ui);
                        ui.allocate_space(ui.available_size());
                    });
                });
        }
    }

    fn show_space(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        objects: &ObjectsBySpace<'_>,
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
            ctx.rec_cfg
                .projected_object_properties
                .get(props.obj_path)
                .visible
        });

        if objects.has_any_3d() {
            let state_3d = self.state_3d.entry(space.cloned()).or_default();
            crate::view3d::combined_view_3d(ctx, ui, state_3d, space, &objects);
        }

        if objects.has_any_2d() {
            let state_2d = self.state_2d.entry(space.cloned()).or_default();
            crate::view2d::combined_view_2d(ctx, ui, state_2d, &objects);
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

    // TODO(emilk): if there are a lot of groups (>3) we likely want to put them in a grid instead of doing a linear split (like we do below)

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
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg: &LogMsg,
    preview: Preview,
) {
    match msg {
        LogMsg::BeginRecordingMsg(msg) => show_begin_recording_msg(ui, msg),
        LogMsg::TypeMsg(msg) => show_type_msg(ctx, ui, msg),
        LogMsg::DataMsg(msg) => {
            show_data_msg(ctx, ui, msg, preview);
        }
    }
}

pub(crate) fn show_begin_recording_msg(ui: &mut egui::Ui, msg: &BeginRecordingMsg) {
    ui.code("BeginRecordingMsg");
    let BeginRecordingMsg { msg_id: _, info } = msg;
    let RecordingInfo {
        recording_id,
        started,
        recording_source,
    } = info;

    egui::Grid::new("fields")
        .striped(true)
        .num_columns(2)
        .show(ui, |ui| {
            ui.monospace("recording_id:");
            ui.label(format!("{recording_id:?}"));
            ui.end_row();

            ui.monospace("started:");
            ui.label(started.format());
            ui.end_row();

            ui.monospace("recording_source:");
            ui.label(format!("{recording_source}"));
            ui.end_row();
        });
}

pub(crate) fn show_type_msg(ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui, msg: &TypeMsg) {
    ui.horizontal(|ui| {
        ctx.type_path_button(ui, &msg.type_path);
        ui.label(" = ");
        ui.code(format!("{:?}", msg.object_type));
    });
}

pub(crate) fn show_data_msg(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg: &DataMsg,
    preview: Preview,
) {
    let DataMsg {
        msg_id,
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
            ui_time_point(ctx, ui, time_point);
            ui.end_row();

            ui.monospace("data:");
            ui_logged_data(ctx, ui, msg_id, data, preview);
            ui.end_row();
        });
}

pub(crate) fn ui_time_point(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    time_point: &TimePoint,
) {
    ui.vertical(|ui| {
        egui::Grid::new("time_point").num_columns(2).show(ui, |ui| {
            for (time_source, value) in &time_point.0 {
                ui.label(format!("{}:", time_source.name()));
                ctx.time_button(ui, time_source, value.as_int());
                ui.end_row();
            }
        });
    });
}

pub(crate) fn ui_logged_data(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg_id: &MsgId,
    data: &LoggedData,
    preview: Preview,
) -> egui::Response {
    match data {
        LoggedData::Batch { data, .. } => ui.label(format!("batch: {:?}", data)),
        LoggedData::Single(data) => ui_data(ctx, ui, msg_id, data, preview),
        LoggedData::BatchSplat(data) => {
            ui.horizontal(|ui| {
                ui.label("Batch Splat:");
                ui_data(ctx, ui, msg_id, data, preview)
            })
            .response
        }
    }
}

pub(crate) fn ui_data(
    ctx: &mut ViewerContext<'_>,
    ui: &mut egui::Ui,
    msg_id: &MsgId,
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
        Data::String(string) => ui.label(format!("{string:?}")),

        Data::Vec2([x, y]) => ui.label(format!("[{x:.1}, {y:.1}]")),
        Data::LineSegments2D(linesegments) => {
            ui.label(format!("{} 2D line segment(s)", linesegments.len()))
        }
        Data::BBox2D(bbox) => ui.label(format!(
            "BBox2D(min: [{:.1} {:.1}], max: [{:.1} {:.1}])",
            bbox.min[0], bbox.min[1], bbox.max[0], bbox.max[1]
        )),

        Data::Vec3([x, y, z]) => ui.label(format!("[{x:.3}, {y:.3}, {z:.3}]")),
        Data::Box3(_) => ui.label("3D box"),
        Data::Path3D(_) => ui.label("3D path"),
        Data::LineSegments3D(segments) => ui.label(format!("{} 3D line segments", segments.len())),
        Data::Mesh3D(_) => ui.label("3D mesh"),
        Data::Camera(_) => ui.label("Camera"),

        Data::Tensor(tensor) => {
            let egui_image = ctx.cache.image.get(msg_id, tensor);
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

                ui.vertical(|ui| {
                    ui.set_min_width(100.0);
                    ui.label(format!("dtype: {:?}", tensor.dtype));

                    if tensor.shape.len() == 2 {
                        ui.label(format!("shape: {:?} (height, width)", tensor.shape));
                    } else if tensor.shape.len() == 3 {
                        ui.label(format!("shape: {:?} (height, width, depth)", tensor.shape));
                    } else {
                        ui.label(format!("shape: {:?}", tensor.shape));
                    }
                });
            })
            .response
        }

        Data::Space(space) => {
            // ui.label(space.to_string())
            ctx.space_button(ui, space)
        }
    }
}
