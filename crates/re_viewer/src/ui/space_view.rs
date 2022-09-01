use std::ops::RangeInclusive;

use egui::{Rect, Vec2};
use itertools::Itertools as _;

use re_data_store::ObjectsBySpace;
use re_log_types::*;

use crate::{misc::HoveredSpace, Preview, Selection, ViewerContext};

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
            .map(|space_path| space_path.to_components())
            .unwrap_or_default()
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone)]
struct Tab {
    space: Option<ObjPath>,
}

fn initialize_tree(tabs: Vec<Tab>) -> egui_dock::Tree<Tab> {
    // TODO: create splits etc
    egui_dock::Tree::new(tabs)
}

struct TabViewer<'a, 'b> {
    ctx: &'a mut ViewerContext<'b>,
    objects: ObjectsBySpace<'b>,
    space_states: &'a mut SpaceStates,
}

impl<'a, 'b> egui_dock::TabViewer for TabViewer<'a, 'b> {
    type Tab = Tab;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        self.space_states
            .show_space(self.ctx, &self.objects, tab.space.as_ref(), ui);
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        space_name(tab.space.as_ref()).into()
    }
}

// ----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct SpaceView {
    // per space
    space_states: SpaceStates,

    #[serde(skip)] // TODO: serialize tab state
    tree: egui_dock::Tree<Tab>,
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

        if self.tree.is_empty() {
            let tabs = all_spaces
                .iter()
                .map(|space| Tab {
                    space: space.cloned(),
                })
                .collect();
            self.tree = initialize_tree(tabs);
        }

        if false {
            ui.label(format!(
                "Spaces: {}",
                all_spaces.iter().map(|&s| space_name(s)).format(" ")
            ));
        }

        let mut tab_viewer = TabViewer {
            ctx,
            objects,
            space_states: &mut self.space_states,
        };

        egui_dock::DockArea::new(&mut self.tree)
            .style(egui_dock::Style::from_egui(ui.style().as_ref()))
            .show_inside(ui, &mut tab_viewer);

        // TODO: this
        // if ctx.rec_cfg.hovered_space.space() != tab_viewer.hovered_space.as_ref() {
        //     ctx.rec_cfg.hovered_space = HoveredSpace::None;
        // }
    }
}

// ----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct SpaceStates {
    // per space
    state_2d: ahash::HashMap<Option<ObjPath>, crate::view2d::State2D>,
    state_3d: ahash::HashMap<Option<ObjPath>, crate::view3d::State3D>,
}

impl SpaceStates {
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

        if objects.has_any_2d() && objects.has_any_3d() {
            log_once::warn_once!("Space {:?} has both 2D and 3D objects", space_name(space));
        }

        if objects.has_any_3d() {
            let state_3d = self.state_3d.entry(space.cloned()).or_default();
            let response = crate::view3d::view_3d(ctx, ui, state_3d, space, &objects);
            if !response.hovered() && ctx.rec_cfg.hovered_space.space() == space {
                ctx.rec_cfg.hovered_space = HoveredSpace::None;
            }
        }

        if objects.has_any_2d() {
            let state_2d = self.state_2d.entry(space.cloned()).or_default();
            let response = crate::view2d::view_2d(ctx, ui, state_2d, space, &objects);
            if !response.hovered() && ctx.rec_cfg.hovered_space.space() == space {
                ctx.rec_cfg.hovered_space = HoveredSpace::None;
            }
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

// ----------------------------------------------------------------------------

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
        Data::BBox2D(bbox) => ui.label(format!(
            "BBox2D(min: [{:.1} {:.1}], max: [{:.1} {:.1}])",
            bbox.min[0], bbox.min[1], bbox.max[0], bbox.max[1]
        )),

        Data::Vec3([x, y, z]) => ui.label(format!("[{x:.3}, {y:.3}, {z:.3}]")),
        Data::Box3(_) => ui.label("3D box"),
        Data::Mesh3D(_) => ui.label("3D mesh"),
        Data::Camera(cam) => match preview {
            Preview::Small | Preview::Specific(_) => ui.label("Camera"),
            Preview::Medium => ui_camera(ui, cam),
        },

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

        Data::DataVec(data_vec) => ui_data_vec(ui, data_vec),
    }
}

pub(crate) fn ui_data_vec(ui: &mut egui::Ui, data_vec: &DataVec) -> egui::Response {
    ui.label(format!(
        "{} x {:?}",
        data_vec.len(),
        data_vec.element_data_type(),
    ))
}

fn ui_camera(ui: &mut egui::Ui, cam: &Camera) -> egui::Response {
    let Camera {
        rotation,
        position,
        camera_space_convention,
        intrinsics,
        resolution,
        target_space,
    } = cam;
    ui.vertical(|ui| {
        ui.label("Camera");
        ui.indent("camera", |ui| {
            egui::Grid::new("camera")
                .striped(true)
                .num_columns(2)
                .show(ui, |ui| {
                    ui.label("rotation");
                    ui.monospace(format!("{rotation:?}"));
                    ui.end_row();

                    ui.label("position");
                    ui.monospace(format!("{position:?}"));
                    ui.end_row();

                    ui.label("camera_space_convention");
                    ui.monospace(format!("{camera_space_convention:?}"));
                    ui.end_row();

                    ui.label("intrinsics");
                    if let Some(intrinsics) = intrinsics {
                        ui_intrinsics(ui, intrinsics);
                    }
                    ui.end_row();

                    ui.label("resolution");
                    ui.monospace(format!("{resolution:?}"));
                    ui.end_row();

                    ui.label("target_space");
                    if let Some(target_space) = target_space {
                        ui.monospace(target_space.to_string());
                    }
                    ui.end_row();
                });
        });
    })
    .response
}

fn ui_intrinsics(ui: &mut egui::Ui, intrinsics: &[[f32; 3]; 3]) {
    egui::Grid::new("intrinsics").num_columns(3).show(ui, |ui| {
        ui.monospace(intrinsics[0][0].to_string());
        ui.monospace(intrinsics[1][0].to_string());
        ui.monospace(intrinsics[2][0].to_string());
        ui.end_row();

        ui.monospace(intrinsics[0][1].to_string());
        ui.monospace(intrinsics[1][1].to_string());
        ui.monospace(intrinsics[2][1].to_string());
        ui.end_row();

        ui.monospace(intrinsics[0][2].to_string());
        ui.monospace(intrinsics[1][2].to_string());
        ui.monospace(intrinsics[2][2].to_string());
        ui.end_row();
    });
}
