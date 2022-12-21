use macaw::BoundingBox;
use re_data_store::{InstanceId, InstanceIdHash, ObjPath, ObjectsProperties};
use re_log_types::Transform;

use crate::misc::{
    space_info::{SpaceInfo, SpacesInfo},
    ViewerContext,
};

use super::{ui_2d::View2DState, ui_3d::View3DState, SceneSpatial, SpaceCamera3D, SpaceSpecs};

/// Describes how the scene is navigated, determining if it is a 2D or 3D experience.
#[derive(Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum SpatialNavigationMode {
    #[default]
    TwoD,
    ThreeD,
}

impl SpatialNavigationMode {
    fn to_ui_string(&self) -> &'static str {
        match self {
            SpatialNavigationMode::TwoD => "2D Pan & Zoom",
            SpatialNavigationMode::ThreeD => "3D Camera",
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

    state_2d: View2DState,
    state_3d: View3DState,
}

impl Default for ViewSpatialState {
    fn default() -> Self {
        Self {
            hovered_instance: Default::default(),
            nav_mode: Default::default(),
            scene_bbox_accum: BoundingBox::nothing(),
            state_2d: Default::default(),
            state_3d: Default::default(),
        }
    }
}

impl ViewSpatialState {
    pub fn settings_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        egui::ComboBox::from_label("Navigation Mode")
            .selected_text(self.nav_mode.to_ui_string())
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut self.nav_mode,
                    SpatialNavigationMode::TwoD,
                    SpatialNavigationMode::TwoD.to_ui_string(),
                );
                ui.selectable_value(
                    &mut self.nav_mode,
                    SpatialNavigationMode::ThreeD,
                    SpatialNavigationMode::ThreeD.to_ui_string(),
                );
            });

        ui.separator();

        let BoundingBox { min, max } = self.scene_bbox_accum;

        match self.nav_mode {
            SpatialNavigationMode::TwoD => {
                ui.label(format!(
                    "Bounding box, x: [{} - {}], y: [{} - {}]",
                    min.x, max.x, min.y, max.y,
                ));
            }
            SpatialNavigationMode::ThreeD => {
                ui.label(format!(
                    "Bounding box, x: [{} - {}], y: [{} - {}], z: [{} - {}]",
                    min.x, max.x, min.y, max.y, min.z, max.z
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
                    &mut self.state_3d,
                    space,
                    scene,
                    space_cameras,
                    &self.scene_bbox_accum,
                    &mut self.hovered_instance,
                    objects_properties,
                )
            }
            SpatialNavigationMode::TwoD => {
                let scene_rect_accum = egui::Rect::from_min_max(
                    self.scene_bbox_accum.min.truncate().to_array().into(),
                    self.scene_bbox_accum.max.truncate().to_array().into(),
                );
                super::view_2d(
                    ctx,
                    ui,
                    &mut self.state_2d,
                    space,
                    scene,
                    scene_rect_accum,
                    &mut self.hovered_instance,
                )
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
                .spaces
                .get(child_path)
                .and_then(|child| child.coordinates);

            if let Some(child_space_info) = spaces_info.spaces.get(child_path) {
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
