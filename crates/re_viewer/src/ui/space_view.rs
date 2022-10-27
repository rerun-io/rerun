use re_data_store::{ObjPath, ObjectTree, ObjectTreeProperties, Objects};
use re_log_types::Transform;

use crate::misc::{space_info::*, ViewerContext};

use super::view3d::SpaceCamera;

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
    view_state: ViewState,

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

    pub fn objects_ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
        space_info: &SpaceInfo,
        time_objects: &Objects<'_>,
        sticky_objects: &Objects<'_>,
    ) -> egui::Response {
        crate::profile_function!();

        let multidim_tensor = multidim_tensor(time_objects);
        let has_2d =
            time_objects.has_any_2d() && (multidim_tensor.is_none() || time_objects.len() > 1);
        let has_3d = time_objects.has_any_3d();
        let has_text = sticky_objects.has_any_text_entries();

        let mut categories = vec![];
        if has_2d {
            categories.push(ViewCategory::TwoD);
        }
        if has_3d {
            categories.push(ViewCategory::ThreeD);
        }
        if multidim_tensor.is_some() {
            categories.push(ViewCategory::Tensor);
        }
        if has_text {
            categories.push(ViewCategory::Text);
        }

        match categories.len() {
            0 => ui.label("(empty)"),
            1 => {
                if has_2d {
                    self.view_state
                        .ui_2d(ctx, ui, &self.space_path, time_objects)
                } else if has_3d {
                    self.view_state.ui_3d(
                        ctx,
                        ui,
                        &self.space_path,
                        spaces_info,
                        space_info,
                        time_objects,
                    )
                } else if let Some(multidim_tensor) = multidim_tensor {
                    self.view_state.ui_tensor(ui, multidim_tensor)
                } else {
                    self.view_state.ui_text(ctx, ui, sticky_objects)
                }
            }
            _ => {
                // Show tabs to let user select which category to view
                ui.vertical(|ui| {
                    if !categories.contains(&self.selected_category) {
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
                        ViewCategory::TwoD => {
                            self.view_state
                                .ui_2d(ctx, ui, &self.space_path, time_objects);
                        }
                        ViewCategory::ThreeD => {
                            self.view_state.ui_3d(
                                ctx,
                                ui,
                                &self.space_path,
                                spaces_info,
                                space_info,
                                time_objects,
                            );
                        }
                        ViewCategory::Tensor => {
                            self.view_state.ui_tensor(ui, multidim_tensor.unwrap());
                        }
                        ViewCategory::Text => {
                            self.view_state.ui_text(ctx, ui, sticky_objects);
                        }
                    }
                })
                .response
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// Camera position and similar.
#[derive(Default, serde::Deserialize, serde::Serialize)]
struct ViewState {
    // per space
    state_2d: crate::view2d::State2D,

    state_3d: crate::view3d::State3D,

    state_tensor: Option<crate::view_tensor::TensorViewState>,

    state_text_entry: crate::text_entry_view::TextEntryState,
}

impl ViewState {
    fn ui_2d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        objects: &Objects<'_>,
    ) -> egui::Response {
        crate::view2d::view_2d(ctx, ui, &mut self.state_2d, Some(space), objects)
    }

    fn ui_3d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        spaces_info: &SpacesInfo,
        space_info: &SpaceInfo,
        objects: &Objects<'_>,
    ) -> egui::Response {
        ui.vertical(|ui| {
            let state = &mut self.state_3d;
            let space_cameras = &space_cameras(spaces_info, space_info);
            let coordinates = space_info.coordinates;
            let space_specs = crate::view3d::SpaceSpecs::from_view_coordinates(coordinates);
            let scene = crate::view3d::scene::Scene::from_objects(ctx, objects);
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

    fn ui_tensor(&mut self, ui: &mut egui::Ui, tensor: &re_log_types::Tensor) -> egui::Response {
        let state_tensor = self
            .state_tensor
            .get_or_insert_with(|| crate::ui::view_tensor::TensorViewState::create(tensor));
        ui.vertical(|ui| {
            crate::view_tensor::view_tensor(ui, state_tensor, tensor);
        })
        .response
    }

    fn ui_text(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        objects: &Objects<'_>,
    ) -> egui::Response {
        self.state_text_entry.show(ui, ctx, objects)
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

fn multidim_tensor<'s>(objects: &Objects<'s>) -> Option<&'s re_log_types::Tensor> {
    // We have a special tensor viewer that (currently) only works
    // when we only have a single tensor (and no bounding boxes etc).
    // It is also not as great for images as the normal 2d view (at least not yet).
    // This is a hacky-way of detecting this special case.
    // TODO(emilk): integrate the tensor viewer into the 2D viewer instead,
    // so we can stack bounding boxes etc on top of it.
    if objects.image.len() == 1 {
        let image = objects.image.first().unwrap().1;
        let tensor = image.tensor;

        // Ignore tensors that likely represent images.
        if tensor.num_dim() > 3 || tensor.num_dim() == 3 && tensor.shape.last().unwrap().size > 4 {
            return Some(tensor);
        }
    }
    None
}
