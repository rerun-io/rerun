use egui::WidgetText;
use macaw::BoundingBox;
use re_data_store::{InstanceId, InstanceIdHash, ObjPath, ObjectsProperties};
use re_format::format_f32;
use re_log_types::Transform;

use crate::misc::{
    space_info::{SpaceInfo, SpacesInfo},
    ViewerContext,
};

use super::{ui_2d::View2DState, ui_3d::View3DState, SceneSpatial, SpaceCamera3D, SpaceSpecs};

/// Describes how the scene is navigated, determining if it is a 2D or 3D experience.
#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum SpatialNavigationMode {
    #[default]
    TwoD,
    ThreeD,
}

impl From<SpatialNavigationMode> for WidgetText {
    fn from(val: SpatialNavigationMode) -> Self {
        match val {
            SpatialNavigationMode::TwoD => "2D Pan & Zoom".into(),
            SpatialNavigationMode::ThreeD => "3D Camera".into(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AutoSizeUnit {
    Auto,
    UiPoints,
    World,
}
impl From<AutoSizeUnit> for WidgetText {
    fn from(val: AutoSizeUnit) -> Self {
        match val {
            AutoSizeUnit::Auto => "auto".into(),
            AutoSizeUnit::UiPoints => "points".into(),
            AutoSizeUnit::World => "units".into(),
        }
    }
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct ViewSpatialState {
    /// What the mouse is hovering (from previous frame)
    #[serde(skip)]
    pub hovered_instance: Option<InstanceId>,

    /// How the scene is navigated.
    pub nav_mode: SpatialNavigationMode,

    /// Estimated bounding box of all data. Accumulated over every time data is displayed.
    #[serde(skip)]
    pub scene_bbox_accum: BoundingBox,

    pub(super) state_2d: View2DState,
    pub(super) state_3d: View3DState,

    /// Size of automatically sized objects. None if it wasn't configured.
    auto_size_config: Option<re_renderer::Size>,
}

impl Default for ViewSpatialState {
    fn default() -> Self {
        Self {
            hovered_instance: Default::default(),
            nav_mode: Default::default(),
            scene_bbox_accum: BoundingBox::nothing(),
            state_2d: Default::default(),
            state_3d: Default::default(),
            auto_size_config: None,
        }
    }
}

impl ViewSpatialState {
    pub fn auto_size_config(&self) -> re_renderer::Size {
        self.auto_size_config
            .unwrap_or_else(|| match self.nav_mode {
                SpatialNavigationMode::TwoD => {
                    re_renderer::Size::new_points(self.default_auto_size_points())
                }
                SpatialNavigationMode::ThreeD => {
                    re_renderer::Size::new_scene(self.default_auto_size_world())
                }
            })
    }

    #[allow(clippy::unused_self)]
    fn default_auto_size_points(&self) -> f32 {
        2.0 // TODO(andreas): Screen size dependent?
    }

    fn default_auto_size_world(&self) -> f32 {
        self.scene_bbox_accum.size().max_element() * 0.0025
    }

    pub fn settings_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Auto size:");

            let (mut displayed_size, mut mode, drag_speed) = match self.auto_size_config {
                None => (self.auto_size_config().0.abs(), AutoSizeUnit::Auto, 0.0),
                Some(size) => {
                    if size.points().is_some() {
                        (size.0.abs(), AutoSizeUnit::UiPoints, 0.25)
                    } else {
                        (
                            size.0.abs(),
                            AutoSizeUnit::World,
                            self.default_auto_size_world() * 0.01,
                        )
                    }
                }
            };

            if ui
                .add_enabled(
                    !matches!(mode, AutoSizeUnit::Auto),
                    egui::DragValue::new(&mut displayed_size)
                        .clamp_range(0.0..=f32::INFINITY)
                        .max_decimals(4)
                        .speed(drag_speed),
                )
                .changed()
            {
                self.auto_size_config = match mode {
                    AutoSizeUnit::Auto => self.auto_size_config, // Shouldn't happen since the DragValue is disabled
                    AutoSizeUnit::UiPoints => Some(re_renderer::Size::new_points(displayed_size)),
                    AutoSizeUnit::World => Some(re_renderer::Size::new_scene(displayed_size)),
                };
            }

            let mode_before = mode;
            egui::ComboBox::from_id_source("auto_size_mode")
                .width(80.0)
                .selected_text(mode)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut mode, AutoSizeUnit::Auto, AutoSizeUnit::Auto)
                        .on_hover_text("Determine automatically.");
                    ui.selectable_value(&mut mode, AutoSizeUnit::UiPoints, AutoSizeUnit::UiPoints)
                        .on_hover_text("Manual in ui points.");
                    ui.selectable_value(&mut mode, AutoSizeUnit::World, AutoSizeUnit::World)
                        .on_hover_text("Manual in scene units.");
                });
            if mode != mode_before {
                self.auto_size_config = match mode {
                    AutoSizeUnit::Auto => None,
                    AutoSizeUnit::UiPoints => Some(re_renderer::Size::new_points(
                        self.default_auto_size_points(),
                    )),
                    AutoSizeUnit::World => {
                        Some(re_renderer::Size::new_scene(self.default_auto_size_world()))
                    }
                }
            }
        })
        .response
        .on_hover_text("Size/radius used whenever not explicitly specified.");

        egui::ComboBox::from_label("Navigation Mode")
            .selected_text(self.nav_mode)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.nav_mode,
                    SpatialNavigationMode::TwoD,
                    SpatialNavigationMode::TwoD,
                );
                ui.selectable_value(
                    &mut self.nav_mode,
                    SpatialNavigationMode::ThreeD,
                    SpatialNavigationMode::ThreeD,
                );
            });

        ui.separator();

        let BoundingBox { min, max } = self.scene_bbox_accum;

        match self.nav_mode {
            SpatialNavigationMode::TwoD => {
                ui.label(format!(
                    "Bounding box:\n  x: [{} - {}]\n  y: [{} - {}]",
                    format_f32(min.x),
                    format_f32(max.x),
                    format_f32(min.y),
                    format_f32(max.y),
                ));
            }
            SpatialNavigationMode::ThreeD => {
                ui.label(format!(
                    "Bounding box:\n  x: [{} - {}]\n  y: [{} - {}]\n  z: [{} - {}]",
                    format_f32(min.x),
                    format_f32(max.x),
                    format_f32(min.y),
                    format_f32(max.y),
                    format_f32(min.z),
                    format_f32(max.z)
                ));
                self.state_3d.settings_ui(ctx, ui, &self.scene_bbox_accum);
            }
        }
    }

    pub fn hovered_instance_hash(&self) -> InstanceIdHash {
        self.hovered_instance
            .as_ref()
            .map_or(InstanceIdHash::NONE, |i| i.hash())
    }

    // TODO(andreas): split into smaller parts, some of it shouldn't be part of the ui path and instead scene loading.
    #[allow(clippy::too_many_arguments)]
    pub fn view_spatial(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        scene: SceneSpatial,
        spaces_info: &SpacesInfo,
        space_info: &SpaceInfo,
        objects_properties: &ObjectsProperties,
    ) -> egui::Response {
        self.scene_bbox_accum = self.scene_bbox_accum.union(scene.primitives.bounding_box());

        match self.nav_mode {
            SpatialNavigationMode::ThreeD => {
                let space_cameras = &space_cameras(spaces_info, space_info);
                let coordinates = space_info.coordinates;
                self.state_3d.space_specs = SpaceSpecs::from_view_coordinates(coordinates);

                super::view_3d(
                    ctx,
                    ui,
                    self,
                    space,
                    scene,
                    space_cameras,
                    objects_properties,
                )
            }
            SpatialNavigationMode::TwoD => {
                let scene_rect_accum = egui::Rect::from_min_max(
                    self.scene_bbox_accum.min.truncate().to_array().into(),
                    self.scene_bbox_accum.max.truncate().to_array().into(),
                );
                super::view_2d(ctx, ui, self, space, scene, scene_rect_accum)
            }
        }
    }

    pub fn help_text(&self) -> &str {
        match self.nav_mode {
            SpatialNavigationMode::TwoD => super::ui_2d::HELP_TEXT,
            SpatialNavigationMode::ThreeD => super::ui_3d::HELP_TEXT,
        }
    }
}

/// Look for camera transform and pinhole in the transform hierarchy
/// and return them as cameras.
fn space_cameras(spaces_info: &SpacesInfo, space_info: &SpaceInfo) -> Vec<SpaceCamera3D> {
    crate::profile_function!();

    let mut space_cameras = vec![];

    for (child_path, child_transform) in &space_info.child_spaces {
        if let Transform::Rigid3(world_from_camera) = child_transform {
            let world_from_camera = world_from_camera.parent_from_child();

            let view_space = spaces_info
                .get(child_path)
                .and_then(|child| child.coordinates);

            if let Some(child_space_info) = spaces_info.get(child_path) {
                for (grand_child_path, grand_child_transform) in &child_space_info.child_spaces {
                    if let Transform::Pinhole(pinhole) = grand_child_transform {
                        space_cameras.push(SpaceCamera3D {
                            camera_obj_path: child_path.clone(),
                            instance_index_hash: re_log_types::IndexHash::NONE,
                            camera_view_coordinates: view_space,
                            world_from_camera,
                            pinhole: Some(*pinhole),
                            target_space: Some(grand_child_path.clone()),
                        });
                    }
                }
            }
        }
    }

    space_cameras
}
