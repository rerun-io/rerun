use ahash::HashSet;
use glam::Vec3;
use re_data_store::{
    InstanceIdHash, ObjPath, ObjectTree, ObjectTreeProperties, Objects, TimeQuery, Timeline,
};
use re_log_types::{MsgId, Tensor, Transform};

use crate::misc::{space_info::*, ViewerContext};

use super::view2d::Scene2d;
use super::view3d::{scene::Scene as Scene3d, scene::Size, SpaceCamera};
use super::views::{
    view_tensor, view_text_entry, SceneTensor, SceneText, TensorViewState, ViewTextEntryState,
};

// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
enum ViewCategory {
    TwoD,
    #[default]
    ThreeD,
    Tensor,
    Text,
}

// ----------------------------------------------------------------------------

/// A view of a space.
#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct SpaceView {
    pub name: String,
    pub space_path: ObjPath,
    pub view_state: ViewState,

    /// In case we are a mix of 2d/3d/tensor/text, we show what?
    selected_category: ViewCategory,

    pub obj_tree_properties: ObjectTreeProperties,
}

impl SpaceView {
    pub fn from_path(space_path: ObjPath) -> Self {
        Self {
            name: space_path.to_string(),
            space_path,
            view_state: Default::default(),
            selected_category: Default::default(),
            obj_tree_properties: Default::default(),
        }
    }

    pub fn on_frame_start(&mut self, obj_tree: &ObjectTree) {
        self.obj_tree_properties.on_frame_start(obj_tree);
    }

    pub(crate) fn scene_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
        space_info: &SpaceInfo,
        time_objects: &Objects<'_>,
        sticky_objects: &Objects<'_>,
        scene: Scene,
    ) -> egui::Response {
        let has_2d = !scene.two_d.is_empty() && (scene.tensor.is_empty() || time_objects.len() > 1);
        let has_3d = !scene.three_d.is_empty();
        let has_text = !scene.text.is_empty();
        let has_tensor = !scene.tensor.is_empty();
        let categories = [
            has_2d.then_some(ViewCategory::TwoD),
            has_3d.then_some(ViewCategory::ThreeD),
            has_text.then_some(ViewCategory::Text),
            has_tensor.then_some(ViewCategory::Tensor),
        ]
        .iter()
        .filter_map(|cat| *cat)
        .collect::<Vec<_>>();

        match categories.len() {
            0 => ui.label("(empty)"),
            1 => {
                if has_2d {
                    self.view_state
                        .ui_2d(ctx, ui, &self.space_path, &scene.two_d)
                } else if has_3d {
                    self.view_state.ui_3d(
                        ctx,
                        ui,
                        &self.space_path,
                        spaces_info,
                        space_info,
                        scene.three_d,
                    )
                } else if has_tensor {
                    self.view_state.ui_tensor(ui, &scene.tensor)
                } else if has_text {
                    self.view_state.ui_text(ctx, ui, &scene.text)
                } else {
                    ui.label("???") // TODO
                }
            }
            _ => {
                // Show tabs to let user select which category to view
                ui.vertical(|ui| {
                    if !categories.contains(&mut self.selected_category) {
                        self.selected_category = categories[0];
                    }

                    ui.horizontal(|ui| {
                        for category in categories {
                            let text = match category {
                                ViewCategory::TwoD => "2D",
                                ViewCategory::ThreeD => "3D",
                                ViewCategory::Tensor => "Tensor",
                                ViewCategory::Text => "Text",
                            };
                            ui.selectable_value(&mut self.selected_category, category, text);
                            // TODO(emilk): make it look like tabs
                        }
                    });
                    ui.separator();

                    match self.selected_category {
                        ViewCategory::Text => {
                            self.view_state.ui_text(ctx, ui, &scene.text);
                        }
                        ViewCategory::Tensor => {
                            self.view_state.ui_tensor(ui, &scene.tensor);
                        }
                        ViewCategory::TwoD => {
                            self.view_state
                                .ui_2d(ctx, ui, &self.space_path, &scene.two_d);
                        }
                        ViewCategory::ThreeD => {
                            self.view_state.ui_3d(
                                ctx,
                                ui,
                                &self.space_path,
                                spaces_info,
                                space_info,
                                scene.three_d,
                            );
                        }
                    }
                })
                .response
                .into()
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// Camera position and similar.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub(crate) struct ViewState {
    state_2d: crate::view2d::State2D,
    state_3d: crate::view3d::State3D,
    state_tensor: Option<TensorViewState>,
    state_text_entry: ViewTextEntryState,
}

impl ViewState {
    fn ui_2d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        scene: &Scene2d,
    ) -> egui::Response {
        crate::view2d::view_2d(ctx, ui, &mut self.state_2d, Some(space), scene)
    }

    fn ui_3d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        spaces_info: &SpacesInfo,
        space_info: &SpaceInfo,
        scene: Scene3d,
    ) -> egui::Response {
        ui.vertical(|ui| {
            let state = &mut self.state_3d;
            let space_cameras = &space_cameras(spaces_info, space_info);
            let coordinates = space_info.coordinates;
            let space_specs = crate::view3d::SpaceSpecs::from_view_coordinates(coordinates);
            crate::view3d::view_3d(
                ctx,
                ui,
                state,
                Some(space),
                &space_specs,
                scene,
                space_cameras,
            );
        })
        .response
    }

    fn ui_tensor(&mut self, ui: &mut egui::Ui, scene: &SceneTensor) -> egui::Response {
        let tensor = &scene.tensors[0];
        let state_tensor = self
            .state_tensor
            .get_or_insert_with(|| TensorViewState::create(tensor));
        ui.vertical(|ui| {
            view_tensor(ui, state_tensor, tensor);
        })
        .response
    }

    fn ui_text(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        scene: &SceneText,
    ) -> egui::Response {
        view_text_entry(ctx, ui, &mut self.state_text_entry, scene)
    }
}

/// Look for camera transform and pinhole in the transform hierarchy
/// and return them as cameras.
fn space_cameras(spaces_info: &SpacesInfo, space_info: &SpaceInfo) -> Vec<SpaceCamera> {
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
                        space_cameras.push(SpaceCamera {
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
                space_cameras.push(SpaceCamera {
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

// ----------------------------------------------------------------------------

#[derive(Debug)]
pub struct SceneQuery {
    pub objects: HashSet<ObjPath>,
    pub timeline: Timeline,
    pub time_query: TimeQuery<i64>,
}

#[derive(Default)]
pub struct Scene {
    pub two_d: Scene2d,
    pub three_d: Scene3d,
    pub text: SceneText,
    pub tensor: SceneTensor,
}

impl ViewState {
    // TODO: temporary
    pub(crate) fn load_scene_from_objects(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        time_objects: &re_data_store::Objects<'_>,
        sticky_objects: &re_data_store::Objects<'_>,
        scene: &mut Scene,
    ) {
        let Scene {
            two_d,
            three_d,
            text,
            tensor,
        } = scene;

        two_d.load_objects(ctx, &self.state_2d, time_objects);
        two_d.load_objects(ctx, &self.state_2d, sticky_objects);

        three_d.load_objects(ctx, time_objects);
        three_d.load_objects(ctx, sticky_objects);

        text.load_objects(ctx, time_objects);
        text.load_objects(ctx, sticky_objects);

        tensor.load_objects(ctx, time_objects);
        tensor.load_objects(ctx, sticky_objects);
    }
}
