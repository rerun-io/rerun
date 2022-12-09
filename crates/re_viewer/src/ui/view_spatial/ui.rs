use re_data_store::{InstanceId, InstanceIdHash, ObjPath};
use re_log_types::Transform;

use crate::misc::{
    space_info::{SpaceInfo, SpacesInfo},
    ViewerContext,
};

use super::{ui_2d::View2DState, ui_3d::View3DState, SceneSpatial, SpaceCamera3D, SpaceSpecs};

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

#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct ViewSpatialState {
    /// What the mouse is hovering (from previous frame)
    #[serde(skip)]
    pub hovered_instance: Option<InstanceId>,

    pub nav_mode: SpatialNavigationMode,

    // TODO(andreas): Not pub?
    pub state_2d: View2DState,
    pub state_3d: View3DState,
}

impl ViewSpatialState {
    pub fn show_settings_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
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

        match self.nav_mode {
            SpatialNavigationMode::TwoD => {}
            SpatialNavigationMode::ThreeD => {
                self.state_3d.show_settings_ui(ctx, ui);
            }
        }
    }

    pub fn hovered_instance_hash(&self) -> InstanceIdHash {
        self.hovered_instance
            .as_ref()
            .map_or(InstanceIdHash::NONE, |i| i.hash())
    }

    pub fn view_spatial(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        scene: SceneSpatial,
        spaces_info: &SpacesInfo,
        space_info: &SpaceInfo,
    ) -> egui::Response {
        let hovered_instance_hash = self.hovered_instance_hash();

        match self.nav_mode {
            SpatialNavigationMode::ThreeD => {
                // TODO(andreas): Why is this here and not in the ui?
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
                    &mut self.hovered_instance,
                    hovered_instance_hash,
                )
            }
            SpatialNavigationMode::TwoD => super::view_2d(
                ctx,
                ui,
                &mut self.state_2d,
                space,
                scene,
                &mut self.hovered_instance,
                hovered_instance_hash,
            ),
        }
    }

    pub fn help_text(&self) -> &str {
        match self.nav_mode {
            SpatialNavigationMode::TwoD => super::ui_2d::HELP_TEXT_2D,
            SpatialNavigationMode::ThreeD => super::ui_3d::HELP_TEXT_3D,
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

            let mut found_any_pinhole = false;

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
                        found_any_pinhole = true;
                    }
                }
            }

            if !found_any_pinhole {
                space_cameras.push(SpaceCamera3D {
                    camera_obj_path: child_path.clone(),
                    instance_index_hash: re_log_types::IndexHash::NONE,
                    camera_view_coordinates: view_space,
                    world_from_camera,
                    pinhole: None,
                    target_space: None,
                });
            }
        }
    }

    space_cameras
}
