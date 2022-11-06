use re_data_store::{ObjPath, ObjectTree, ObjectTreeProperties};
use re_log_types::Transform;

use crate::misc::{space_info::*, ViewerContext};

use super::{view_2d, view_3d, view_tensor, view_text, Scene};

// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub(crate) enum ViewCategory {
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
    pub selected_category: ViewCategory,

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
        // NOTE: mutable because the glow-based 3D view `take()`s the scene because reasons.
        // TODO(cmc): remove this while removing glow.
        scene: &mut Scene,
    ) {
        crate::profile_function!();

        let has_2d = !scene.two_d.is_empty() && scene.tensor.is_empty();
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
        // Extra headroom required for the hovering controls at the top of the space view.
        let extra_headroom = {
            let frame = ctx.design_tokens.hovering_frame(ui.style());
            frame.total_margin().sum().y
                + ui.spacing().interact_size.y
                + ui.spacing().item_spacing.y
        };

        match categories.len() {
            0 => {
                ui.label("(empty)");
            }
            1 => {
                self.selected_category = categories[0];
                if has_2d {
                    _ = extra_headroom; // ignored - we just overlay on top of the 2D view.
                    self.view_state
                        .ui_2d(ctx, ui, &self.space_path, &scene.two_d);
                } else if has_3d {
                    _ = extra_headroom; // ignored - we just overlay on top of the 2D view.
                    self.view_state.ui_3d(
                        ctx,
                        ui,
                        &self.space_path,
                        spaces_info,
                        space_info,
                        &mut scene.three_d,
                    );
                } else if has_tensor {
                    ui.add_space(extra_headroom);
                    self.view_state.ui_tensor(ui, &scene.tensor);
                } else {
                    assert!(has_text);
                    ui.add_space(extra_headroom);
                    self.view_state.ui_text(ctx, ui, &scene.text);
                }
            }
            _ => {
                // Show tabs to let user select which category to view
                ui.add_space(extra_headroom);
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
                            &mut scene.three_d,
                        );
                    }
                }
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// Camera position and similar.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub(crate) struct ViewState {
    pub state_2d: view_2d::View2DState,
    pub state_3d: view_3d::View3DState,
    pub state_tensor: Option<view_tensor::ViewTensorState>,
    pub state_text_entry: view_text::ViewTextState,
}

impl ViewState {
    fn ui_2d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        scene: &view_2d::Scene2D,
    ) -> egui::Response {
        let response = ui
            .scope(|ui| {
                view_2d::view_2d(ctx, ui, &mut self.state_2d, Some(space), scene);
            })
            .response;

        // Show help-text on top of space:
        {
            let mut ui = ui.child_ui(response.rect, egui::Layout::right_to_left(egui::Align::TOP));
            ctx.design_tokens
                .hovering_frame(ui.style())
                .show(&mut ui, |ui| {
                    crate::misc::help_hover_button(ui).on_hover_text(view_2d::HELP_TEXT);
                });
        }

        response
    }

    fn ui_3d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        spaces_info: &SpacesInfo,
        space_info: &SpaceInfo,
        scene: &mut view_3d::Scene3D,
    ) -> egui::Response {
        ui.vertical(|ui| {
            let state = &mut self.state_3d;
            let space_cameras = &space_cameras(spaces_info, space_info);
            let coordinates = space_info.coordinates;
            state.space_specs = view_3d::SpaceSpecs::from_view_coordinates(coordinates);
            let response = ui
                .scope(|ui| {
                    view_3d::view_3d(ctx, ui, state, Some(space), scene, space_cameras);
                })
                .response;

            // Show help-text on top of space:
            {
                let mut ui =
                    ui.child_ui(response.rect, egui::Layout::right_to_left(egui::Align::TOP));
                ctx.design_tokens
                    .hovering_frame(ui.style())
                    .show(&mut ui, |ui| {
                        crate::misc::help_hover_button(ui).on_hover_text(view_3d::HELP_TEXT);
                    });
            }

            response
        })
        .response
    }

    fn ui_tensor(&mut self, ui: &mut egui::Ui, scene: &view_tensor::SceneTensor) -> egui::Response {
        let tensor = &scene.tensors[0];
        let state_tensor = self
            .state_tensor
            .get_or_insert_with(|| view_tensor::ViewTensorState::create(tensor));
        ui.vertical(|ui| {
            view_tensor::view_tensor(ui, state_tensor, tensor);
        })
        .response
    }

    fn ui_text(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        scene: &view_text::SceneText,
    ) -> egui::Response {
        view_text::view_text(ctx, ui, &mut self.state_text_entry, scene)
    }
}

/// Look for camera transform and pinhole in the transform hierarchy
/// and return them as cameras.
fn space_cameras(spaces_info: &SpacesInfo, space_info: &SpaceInfo) -> Vec<view_3d::SpaceCamera> {
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
                        space_cameras.push(view_3d::SpaceCamera {
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
                space_cameras.push(view_3d::SpaceCamera {
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
