use nohash_hasher::IntSet;
use re_data_store::{ObjPath, ObjectTree, ObjectTreeProperties, TimeInt};

use crate::misc::{
    space_info::{SpaceInfo, SpacesInfo},
    ViewerContext,
};

use super::{
    view_plot,
    view_spatial::{self, SpatialNavigationMode},
    view_tensor, view_text,
};

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
    #[default]
    Spatial,
    Tensor,
    Text,
    Plot,
}

// ----------------------------------------------------------------------------

/// A view of a space.
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub(crate) struct SpaceView {
    pub name: String,

    /// Everything under this root is shown in the space view.
    pub root_path: ObjPath,

    /// Everything visible in this space view, is looked at in reference to this space info.
    pub reference_space_path: ObjPath,

    pub view_state: ViewState,

    /// We only show data that match this category.
    pub category: ViewCategory,

    pub obj_tree_properties: ObjectTreeProperties,
}

impl SpaceView {
    pub fn new(
        scene: &super::scene::Scene,
        category: ViewCategory,
        reference_space_path: ObjPath,
    ) -> Self {
        let mut view_state = ViewState::default();

        if category == ViewCategory::Spatial {
            view_state.state_spatial.nav_mode = if scene.spatial.prefer_2d_mode() {
                SpatialNavigationMode::TwoD
            } else {
                SpatialNavigationMode::ThreeD
            };
        }

        let root_path = reference_space_path.to_components().first().map_or_else(
            || reference_space_path.clone(),
            |c| ObjPath::from(vec![c.clone()]),
        );

        Self {
            name: reference_space_path.to_string(),
            root_path,
            reference_space_path,
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
        reference_space_info: &SpaceInfo,
        latest_at: TimeInt,
    ) {
        crate::profile_function!();

        let no_transform_query = crate::ui::scene::SceneQuery {
            obj_paths: &reference_space_info.children_without_transform,
            timeline: *ctx.rec_cfg.time_ctrl.timeline(),
            latest_at,
            obj_props: &self.obj_tree_properties.projected,
        };

        match self.category {
            ViewCategory::Spatial => {
                // TODO(andreas): This list is gathered potentially in a bunch of places.
                let mut obj_paths = IntSet::default();
                fn gather_paths(tree: &ObjectTree, obj_paths: &mut IntSet<ObjPath>) {
                    obj_paths.insert(tree.path.clone());
                    for subtree in tree.children.values() {
                        gather_paths(subtree, obj_paths);
                    }
                }
                let Some(root_tree) = ctx.log_db.obj_db.tree.subtree(&self.root_path) else {
                    return;
                };
                gather_paths(root_tree, &mut obj_paths);

                let query = crate::ui::scene::SceneQuery {
                    obj_paths: &obj_paths,
                    timeline: *ctx.rec_cfg.time_ctrl.timeline(),
                    latest_at,
                    obj_props: &self.obj_tree_properties.projected,
                };

                let mut scene = view_spatial::SceneSpatial::default();
                scene.load_objects(
                    ctx,
                    &query,
                    self.view_state.state_spatial.hovered_instance_hash(),
                );
                self.view_state.ui_spatial(
                    ctx,
                    ui,
                    &self.reference_space_path,
                    spaces_info,
                    reference_space_info,
                    scene,
                );
            }

            ViewCategory::Tensor => {
                ui.add_space(16.0); // Extra headroom required for the hovering controls at the top of the space view.

                let mut scene = view_tensor::SceneTensor::default();
                scene.load_objects(ctx, &no_transform_query);
                self.view_state.ui_tensor(ui, &scene);
            }
            ViewCategory::Text => {
                let mut scene = view_text::SceneText::default();
                scene.load_objects(
                    ctx,
                    &no_transform_query,
                    &self.view_state.state_text.filters,
                );
                self.view_state.ui_text(ctx, ui, &scene);
            }
            ViewCategory::Plot => {
                let mut scene = view_plot::ScenePlot::default();
                scene.load_objects(ctx, &no_transform_query);
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
        ctx.re_ui.hovering_frame().show(&mut ui, |ui| {
            crate::misc::help_hover_button(ui).on_hover_text(help_text);
        });
    }
}

// ----------------------------------------------------------------------------

/// Camera position and similar.
#[derive(Clone, Default, serde::Deserialize, serde::Serialize)]
pub(crate) struct ViewState {
    pub state_spatial: view_spatial::ViewSpatialState,
    pub state_tensor: Option<view_tensor::ViewTensorState>,
    pub state_text: view_text::ViewTextState,
    pub state_plot: view_plot::ViewPlotState,
}

impl ViewState {
    fn ui_spatial(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        spaces_info: &SpacesInfo,
        space_info: &SpaceInfo,
        scene: view_spatial::SceneSpatial,
    ) {
        ui.vertical(|ui| {
            let response =
                self.state_spatial
                    .view_spatial(ctx, ui, space, scene, spaces_info, space_info);
            show_help_button_overlay(ui, response.rect, ctx, self.state_spatial.help_text());
        });
    }

    fn ui_tensor(&mut self, ui: &mut egui::Ui, scene: &view_tensor::SceneTensor) {
        if scene.tensors.is_empty() {
            ui.centered_and_justified(|ui| ui.label("(empty)"));
        } else if scene.tensors.len() == 1 {
            let tensor = &scene.tensors[0];
            let state_tensor = self
                .state_tensor
                .get_or_insert_with(|| view_tensor::ViewTensorState::create(tensor));

            egui::Frame {
                inner_margin: re_ui::ReUi::view_padding().into(),
                ..egui::Frame::default()
            }
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    view_tensor::view_tensor(ui, state_tensor, tensor);
                });
            });
        } else {
            ui.centered_and_justified(|ui| {
                ui.label("ERROR: more than one tensor!") // TODO(emilk): in this case we should have one space-view per tensor.
            });
        }
    }

    fn ui_text(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        scene: &view_text::SceneText,
    ) {
        egui::Frame {
            inner_margin: re_ui::ReUi::view_padding().into(),
            ..egui::Frame::default()
        }
        .show(ui, |ui| {
            view_text::view_text(ctx, ui, &mut self.state_text, scene);
        });
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
