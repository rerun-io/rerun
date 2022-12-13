use nohash_hasher::{IntMap, IntSet};
use re_data_store::{ObjPath, ObjectTree, ObjectTreeProperties, TimeInt};

use crate::{
    misc::{
        space_info::{SpaceInfo, SpacesInfo},
        ViewerContext,
    },
    ui::SpaceViewId,
};

use super::{
    view_plot,
    view_spatial::{self, SpatialNavigationMode},
    view_tensor, view_text,
    viewport::visibility_button,
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
    pub id: SpaceViewId,
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
        reference_space: &SpaceInfo,
        obj_tree: &ObjectTree,
    ) -> Self {
        let mut view_state = ViewState::default();

        if category == ViewCategory::Spatial {
            view_state.state_spatial.nav_mode = if scene.spatial.prefer_2d_mode() {
                SpatialNavigationMode::TwoD
            } else {
                SpatialNavigationMode::ThreeD
            };
        }

        let root_path = reference_space_path.iter().next().map_or_else(
            || reference_space_path.clone(),
            |c| ObjPath::from(vec![c.to_owned()]),
        );

        // By default, make everything above and next to the reference path invisible.
        let mut obj_tree_properties = ObjectTreeProperties::default();
        fn hide_non_reference_path_children(
            subtree: &ObjectTree,
            tree_properties: &mut ObjectTreeProperties,
            ref_path: &ObjPath,
            ref_space: &SpaceInfo,
        ) {
            if !subtree.path.is_ancestor_or_child_of(ref_path)
                || (subtree.path.is_child_of(ref_path)
                    && !ref_space.children_without_transform.contains(&subtree.path))
            {
                tree_properties.individual.set(
                    subtree.path.clone(),
                    re_data_store::ObjectProps {
                        visible: false,
                        ..Default::default()
                    },
                );
            } else {
                for child in subtree.children.values() {
                    hide_non_reference_path_children(child, tree_properties, ref_path, ref_space);
                }
            }
        }
        if let Some(subtree) = obj_tree.subtree(&root_path) {
            hide_non_reference_path_children(
                subtree,
                &mut obj_tree_properties,
                &reference_space_path,
                reference_space,
            );
        }

        Self {
            name: root_path.to_string(),
            id: SpaceViewId::random(),
            root_path,
            reference_space_path,
            view_state,
            category,
            obj_tree_properties,
        }
    }

    /// All object paths that are forced to be invisible and why.
    ///
    /// We're not storing this since the circumstances for this may change over time.
    /// (either by choosing a different reference space path or by having new paths added)
    pub fn forcibly_invisible_elements(
        &mut self,
        spaces_info: &SpacesInfo,
        obj_tree: &ObjectTree,
    ) -> IntMap<ObjPath, &'static str> {
        crate::profile_function!();

        let mut forced_invisible = IntMap::default();

        let Some(reference_space) = spaces_info.spaces.get(&self.reference_space_path) else {
            return forced_invisible; // Should never happen?
        };

        // Direct children of the current reference space.
        for (path, transform) in &reference_space.child_spaces {
            match transform {
                re_log_types::Transform::Unknown => {}

                // TODO(andreas): This should be made possible!
                re_log_types::Transform::Rigid3(_) => {
                    forced_invisible.insert(
                        path.clone(),
                        "Can't display elements with a rigid transform relative to the reference path in the same spaceview yet",
                    );
                }

                // TODO(andreas): This should be made possible *iff* the reference space itself doesn't define a pinhole camera (or is there a way to deal with that?)
                re_log_types::Transform::Pinhole(_) => {
                    forced_invisible.insert(
                        path.clone(),
                        "Can't display elements with a pinhole transform relative to the reference path in the same spaceview yet",
                    );
                }
            }
        }

        obj_tree.recurse_siblings_and_aunts(&self.reference_space_path, |sibling| {
            if sibling.parent().unwrap().is_root() {
                return;
            }

            // TODO(andreas): We should support most parent & sibling transforms by applying the inverse transform.
            //                Breaking out of pinhole relationships is going to be a bit harder as it will need extra parameters.
            forced_invisible.insert(
                sibling.clone(),
                "Can't display elements aren't children of the reference path yet.",
            );
        });

        forced_invisible
    }

    pub fn on_frame_start(&mut self, obj_tree: &ObjectTree) {
        self.obj_tree_properties.on_frame_start(obj_tree);
    }

    pub fn selection_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        egui::Grid::new("space_view")
            .striped(re_ui::ReUi::striped())
            .num_columns(2)
            .show(ui, |ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut self.name);
                ui.end_row();

                ui.label("Query Root Path:");
                ctx.obj_path_button(ui, &self.root_path);
                ui.end_row();

                ui.label("Reference Space Path:");
                ctx.obj_path_button(ui, &self.reference_space_path);
                ui.end_row();
            });

        ui.separator();

        ui.strong("Query Tree");
        self.query_tree_ui(ctx, ui);

        ui.separator();

        match self.category {
            ViewCategory::Spatial => {
                ui.strong("Spatial view");
                self.view_state.state_spatial.show_settings_ui(ctx, ui);
            }
            ViewCategory::Tensor => {
                if let Some(state_tensor) = &mut self.view_state.state_tensor {
                    ui.strong("Tensor view");
                    state_tensor.ui(ui);
                }
            }
            ViewCategory::Text => {
                ui.strong("Text view");
                ui.add_space(4.0);
                view_text::text_filters_ui(ui, &mut self.view_state.state_text);
            }
            ViewCategory::Plot => {}
        }
    }

    fn query_tree_ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        let obj_tree = &ctx.log_db.obj_db.tree;

        // We'd like to see the reference space path by default.
        let default_open = self.root_path != self.reference_space_path;
        let collapsing_header_id = ui.make_persistent_id(self.id);
        egui::collapsing_header::CollapsingState::load_with_default_open(
            ui.ctx(),
            collapsing_header_id,
            default_open,
        )
        .show_header(ui, |ui| {
            ui.label(self.root_path.to_string());
        })
        .body(|ui| {
            if let Some(subtree) = obj_tree.subtree(&self.root_path) {
                // TODO(andreas): Recreating this here might be wasteful
                let spaces_info = SpacesInfo::new(&ctx.log_db.obj_db, &ctx.rec_cfg.time_ctrl);

                let forced_invisible = self.forcibly_invisible_elements(&spaces_info, obj_tree);
                self.show_obj_tree_children(ctx, ui, &spaces_info, subtree, &forced_invisible);
            }
        });
    }

    fn show_obj_tree_children(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
        tree: &ObjectTree,
        forced_invisible: &IntMap<ObjPath, &str>,
    ) {
        if tree.children.is_empty() {
            ui.weak("(nothing)");
            return;
        }

        let parent_is_visible = self.obj_tree_properties.individual.get(&tree.path).visible;
        for (path_comp, child_tree) in &tree.children {
            self.show_obj_tree(
                ctx,
                ui,
                spaces_info,
                &path_comp.to_string(),
                child_tree,
                forced_invisible,
                parent_is_visible,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn show_obj_tree(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        spaces_info: &SpacesInfo,
        name: &str,
        tree: &ObjectTree,
        forced_invisible: &IntMap<ObjPath, &str>,
        parent_is_visible: bool,
    ) {
        let disabled_reason = forced_invisible.get(&tree.path);
        let response = ui
            .add_enabled_ui(disabled_reason.is_none(), |ui| {
                if tree.is_leaf() {
                    ui.horizontal(|ui| {
                        self.object_path_button(ctx, ui, &tree.path, spaces_info, name);
                        self.object_visibility_button(ui, parent_is_visible, &tree.path);
                    });
                } else {
                    let collapsing_header_id = ui.id().with(&tree.path);

                    // Default open so that the reference path is visible.
                    let default_open = self.reference_space_path.is_child_of(&tree.path);
                    egui::collapsing_header::CollapsingState::load_with_default_open(
                        ui.ctx(),
                        collapsing_header_id,
                        default_open,
                    )
                    .show_header(ui, |ui| {
                        self.object_path_button(ctx, ui, &tree.path, spaces_info, name);
                        self.object_visibility_button(ui, parent_is_visible, &tree.path);
                    })
                    .body(|ui| {
                        self.show_obj_tree_children(ctx, ui, spaces_info, tree, forced_invisible);
                    });
                }
            })
            .response;

        if let Some(disabled_reason) = disabled_reason {
            response.on_hover_text(*disabled_reason);
        }
    }

    fn object_path_button(
        &self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        path: &ObjPath,
        spaces_info: &SpacesInfo,
        name: &str,
    ) {
        let mut is_space_info = false;
        let label_text = if spaces_info.spaces.contains_key(path) {
            is_space_info = true;
            let label_text = egui::RichText::new(format!("📐 {}", name));
            if *path == self.reference_space_path {
                label_text.strong()
            } else {
                label_text
            }
        } else {
            egui::RichText::new(name)
        };

        if ctx
            .space_view_obj_path_button_to(ui, label_text, self.id, path)
            .double_clicked()
            && is_space_info
        {
            // TODO(andreas): Can't yet change the reference space.
            //*reference_space = path.clone();
        }
    }

    fn object_visibility_button(
        &mut self,
        ui: &mut egui::Ui,
        parent_is_visible: bool,
        path: &ObjPath,
    ) {
        let are_all_ancestors_visible = parent_is_visible
            && match path.parent() {
                None => true, // root
                Some(parent) => self.obj_tree_properties.projected.get(&parent).visible,
            };

        let mut props = self.obj_tree_properties.individual.get(path);
        if visibility_button(ui, are_all_ancestors_visible, &mut props.visible).changed() {
            self.obj_tree_properties.individual.set(path.clone(), props);
        }
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

        // Gather all object paths under the current root that aren't force invisible.
        fn gather_paths(
            tree: &ObjectTree,
            obj_paths: &mut IntSet<ObjPath>,
            excluded_paths: &IntMap<ObjPath, &str>,
        ) {
            if !excluded_paths.contains_key(&tree.path) {
                obj_paths.insert(tree.path.clone());
                for subtree in tree.children.values() {
                    gather_paths(subtree, obj_paths, excluded_paths);
                }
            }
        }
        let Some(root_tree) = ctx.log_db.obj_db.tree.subtree(&self.root_path) else {
            return;
        };
        let excluded_paths = self.forcibly_invisible_elements(spaces_info, &ctx.log_db.obj_db.tree);
        let mut obj_paths = IntSet::default();
        gather_paths(root_tree, &mut obj_paths, &excluded_paths);

        let query = crate::ui::scene::SceneQuery {
            obj_paths: &obj_paths,
            timeline: *ctx.rec_cfg.time_ctrl.timeline(),
            latest_at,
            obj_props: &self.obj_tree_properties.projected,
        };

        match self.category {
            ViewCategory::Spatial => {
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
                scene.load_objects(ctx, &query);
                self.view_state.ui_tensor(ui, &scene);
            }
            ViewCategory::Text => {
                let mut scene = view_text::SceneText::default();
                scene.load_objects(ctx, &query, &self.view_state.state_text.filters);
                self.view_state.ui_text(ctx, ui, &scene);
            }
            ViewCategory::Plot => {
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
