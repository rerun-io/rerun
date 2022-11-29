use re_data_store::{ObjPath, ObjectTree, ObjectTreeProperties, TimeInt};
use re_log_types::Transform;

use crate::misc::{space_info::*, ViewerContext};

use super::{view_2d, view_3d, view_plot, view_tensor, view_text};

// ----------------------------------------------------------------------------

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    serde::Deserialize,
    serde::Serialize,
)]
pub enum ViewCategory {
    TwoD,
    #[default]
    ThreeD,
    Tensor,
    Text,
    Plot,
}

// ----------------------------------------------------------------------------

/// A view of a space.
#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct SpaceView {
    pub name: String,
    pub space_path: ObjPath,
    pub view_state: ViewState,

    /// We only show data that match this category.
    pub category: ViewCategory,

    pub obj_tree_properties: ObjectTreeProperties,
}

impl SpaceView {
    pub fn new(scene: &super::scene::Scene, category: ViewCategory, space_path: ObjPath) -> Self {
        let mut view_state = ViewState::default();

        if category == ViewCategory::TwoD {
            // A good start:
            view_state.state_2d.scene_bbox_accum = scene.two_d.bbox;
        }

        Self {
            name: space_path.to_string(),
            space_path,
            view_state,
            category,
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
        latest_at: TimeInt,
    ) {
        crate::profile_function!();

        let query = crate::ui::scene::SceneQuery {
            obj_paths: &space_info.objects,
            timeline: *ctx.rec_cfg.time_ctrl.timeline(),
            latest_at,
            obj_props: &self.obj_tree_properties.projected,
        };

        // Extra headroom required for the hovering controls at the top of the space view.
        let extra_headroom = {
            let frame = ctx.design_tokens.hovering_frame(ui.style());
            frame.total_margin().sum().y + ui.spacing().interact_size.y
        };

        match self.category {
            ViewCategory::TwoD => {
                _ = extra_headroom; // ignored - put overlay buttons on top of the view.

                let mut scene = view_2d::Scene2D::default();
                scene.load_objects(ctx, &query);
                self.view_state.ui_2d(ctx, ui, &self.space_path, scene);
            }
            ViewCategory::ThreeD => {
                _ = extra_headroom; // ignored - put overlay buttons on top of the view.

                let mut scene = view_3d::Scene3D::default();
                scene.load_objects(ctx, &query);
                self.view_state
                    .ui_3d(ctx, ui, &self.space_path, spaces_info, space_info, scene);
            }
            ViewCategory::Tensor => {
                ui.add_space(extra_headroom);

                let mut scene = view_tensor::SceneTensor::default();
                scene.load_objects(ctx, &query);
                self.view_state.ui_tensor(ui, &scene);
            }
            ViewCategory::Text => {
                let line_height = egui::TextStyle::Body.resolve(ui.style()).size;
                ui.add_space(extra_headroom - line_height - ui.spacing().item_spacing.y); // we don't need the full headroom - the logs has the number of entries at the top

                let mut scene = view_text::SceneText::default();
                scene.load_objects(ctx, &query);
                self.view_state.ui_text(ctx, ui, &scene);
            }
            ViewCategory::Plot => {
                _ = extra_headroom; // ignored - put overlay buttons on top of the view.

                let mut scene = view_plot::ScenePlot::default();
                scene.load_objects(ctx, &query);
                self.view_state.ui_plot(ctx, ui, &scene);
            }
        };
    }
}

// ----------------------------------------------------------------------------

/// Show help-text on top of space
fn show_help_button_overlay(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    ctx: &mut ViewerContext<'_>,
    help_text: &str,
) {
    {
        let mut ui = ui.child_ui(rect, egui::Layout::right_to_left(egui::Align::TOP));
        ctx.design_tokens
            .hovering_frame(ui.style())
            .show(&mut ui, |ui| {
                crate::misc::help_hover_button(ui).on_hover_text(help_text);
            });
    }
}

// ----------------------------------------------------------------------------

/// Camera position and similar.
#[derive(Default, serde::Deserialize, serde::Serialize)]
pub(crate) struct ViewState {
    pub state_2d: view_2d::View2DState,
    pub state_3d: view_3d::View3DState,
    pub state_tensor: Option<view_tensor::ViewTensorState>,
    pub state_text: view_text::ViewTextState,
    pub state_plot: view_plot::ViewPlotState,
}

impl ViewState {
    fn ui_2d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        scene: view_2d::Scene2D,
    ) -> egui::Response {
        let response = ui
            .scope(|ui| {
                view_2d::view_2d(ctx, ui, &mut self.state_2d, Some(space), scene);
            })
            .response;

        show_help_button_overlay(ui, response.rect, ctx, view_2d::HELP_TEXT);

        response
    }

    fn ui_3d(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        spaces_info: &SpacesInfo,
        space_info: &SpaceInfo,
        scene: view_3d::Scene3D,
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

            show_help_button_overlay(ui, response.rect, ctx, view_3d::HELP_TEXT);
        })
        .response
    }

    fn ui_tensor(&mut self, ui: &mut egui::Ui, scene: &view_tensor::SceneTensor) {
        if scene.tensors.is_empty() {
            ui.centered(|ui| ui.label("(empty)"));
        } else if scene.tensors.len() == 1 {
            let tensor = &scene.tensors[0];
            let state_tensor = self
                .state_tensor
                .get_or_insert_with(|| view_tensor::ViewTensorState::create(tensor));
            ui.vertical(|ui| {
                view_tensor::view_tensor(ui, state_tensor, tensor);
            });
        } else {
            ui.centered(|ui| {
                ui.label("ERROR: more than one tensor!") // TODO(emilk): in this case we should have one space-view per tensor.
            });
        }
    }

    fn ui_text(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        scene: &view_text::SceneText,
    ) -> egui::Response {
        view_text::view_text(ctx, ui, &mut self.state_text, scene)
    }

    fn ui_plot(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        scene: &view_plot::ScenePlot,
    ) -> egui::Response {
        ui.vertical(|ui| {
            let response = ui
                .scope(|ui| {
                    view_plot::view_plot(ctx, ui, &mut self.state_plot, scene);
                })
                .response;

            show_help_button_overlay(ui, response.rect, ctx, view_plot::HELP_TEXT);
        })
        .response
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
