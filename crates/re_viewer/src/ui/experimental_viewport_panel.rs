use std::collections::BTreeMap;

use ahash::HashMap;
use itertools::Itertools as _;

use nohash_hasher::IntSet;
use re_data_store::{
    log_db::ObjDb, FieldName, ObjPath, ObjPathComp, ObjectTree, Objects, TimeQuery, Timeline,
    TimelineStore,
};
use re_log_types::{ObjectType, Transform};

use crate::misc::ViewerContext;

// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
struct SpaceViewId(uuid::Uuid);

impl SpaceViewId {
    pub fn random() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

// ----------------------------------------------------------------------------

#[derive(Default)]
struct SpaceInfo {
    /// All paths in this space (including self and children connected by the identity transform).
    objects: IntSet<ObjPath>,

    parent: Option<(ObjPath, Transform)>,
    child_spaces: BTreeMap<ObjPath, Transform>,
}

#[derive(Default)]
struct SpacesInfo {
    spaces: BTreeMap<ObjPath, SpaceInfo>,
}

impl SpacesInfo {
    fn new(obj_db: &ObjDb, timeline: &Timeline) -> Self {
        crate::profile_function!();

        fn add_children(
            timeline_store: Option<&TimelineStore<i64>>,
            spaces_info: &mut SpacesInfo,
            parent_space_path: &ObjPath,
            parent_space_info: &mut SpaceInfo,
            tree: &ObjectTree,
        ) {
            if let Some(transform) = query_transform(timeline_store, &tree.path) {
                parent_space_info
                    .child_spaces
                    .insert(tree.path.clone(), transform.clone());

                let mut child_space_info = SpaceInfo {
                    parent: Some((parent_space_path.clone(), transform.clone())),
                    ..Default::default()
                };
                child_space_info.objects.insert(tree.path.clone()); // spaces includes self

                for child_tree in tree.children.values() {
                    add_children(
                        timeline_store,
                        spaces_info,
                        &tree.path,
                        &mut child_space_info,
                        child_tree,
                    );
                }
                spaces_info
                    .spaces
                    .insert(tree.path.clone(), child_space_info);
            } else {
                // no transform == identity transform.
                parent_space_info.objects.insert(tree.path.clone()); // spaces includes self

                for child_tree in tree.children.values() {
                    add_children(
                        timeline_store,
                        spaces_info,
                        parent_space_path,
                        parent_space_info,
                        child_tree,
                    );
                }
            }
        }

        let timeline_store = obj_db.store.get(timeline);

        let mut spaces_info = Self::default();

        for tree in obj_db.tree.children.values() {
            // Each root object is its own space (or should be)

            if query_transform(timeline_store, &tree.path).is_some() {
                re_log::warn_once!(
                    "Root object '{}' has a _transform - this is not allowed!",
                    tree.path
                );
            }

            let mut space_info = SpaceInfo::default();
            add_children(
                timeline_store,
                &mut spaces_info,
                &tree.path,
                &mut space_info,
                tree,
            );
            spaces_info.spaces.insert(tree.path.clone(), space_info);
        }

        spaces_info
    }
}

// ----------------------------------------------------------------------------

/// Get the latest value of the "_transform" meta-field of the given object.
fn query_transform<'s>(
    store: Option<&'s TimelineStore<i64>>,
    obj_path: &ObjPath,
) -> Option<&'s re_log_types::Transform> {
    let field_store = store?.get(obj_path)?.get(&FieldName::from("_transform"))?;
    // `_transform` is only allowed to be stored in a mono-field.
    let mono_field_store = field_store.get_mono::<re_log_types::Transform>().ok()?;
    Some(&mono_field_store.latest()?.1 .1)
}

// ----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct Blueprint {
    tree: egui_dock::Tree<SpaceViewId>,

    /// Show one tab as maximized?
    maximized: Option<SpaceViewId>,

    space_views: HashMap<SpaceViewId, SpaceView>,
}

impl Blueprint {
    pub fn new(spaces_info: &SpacesInfo, available_size: egui::Vec2) -> Self {
        crate::profile_function!();

        let mut blueprint = Self::default();

        let mut space_make_infos = vec![];

        for path in spaces_info.spaces.keys() {
            let space_view_id = SpaceViewId::random();
            blueprint.space_views.insert(
                space_view_id,
                SpaceView {
                    name: path.to_string(),
                    space_path: path.clone(),
                    view_state: ViewState::default(),
                },
            );
            space_make_infos.push(SpaceMakeInfo {
                id: space_view_id,
                path: path.clone(),
                size2d: None, // TODO
            });
        }

        let layout = layout_spaces(available_size, &mut space_make_infos);
        blueprint.tree = egui_dock::Tree::new(vec![]);
        tree_from_split(&mut blueprint.tree, egui_dock::NodeIndex(0), &layout);

        blueprint
    }

    pub fn tree_ui(&mut self, ui: &mut egui::Ui, obj_tree: &ObjectTree) {
        ui.heading("Blueprint");

        let focused = self.tree.find_active_focused().map(|(_, id)| *id);

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for (space_view_id, space_view) in self
                    .space_views
                    .iter()
                    .sorted_by_key(|(_, space_view)| &space_view.name)
                {
                    let is_focused = Some(*space_view_id) == focused;

                    let collapsing_header_id = ui.make_persistent_id(&space_view_id);
                    let default_open = true;
                    egui::collapsing_header::CollapsingState::load_with_default_open(
                        ui.ctx(),
                        collapsing_header_id,
                        default_open,
                    )
                    .show_header(ui, |ui| {
                        if ui.selectable_label(is_focused, &space_view.name).clicked() {
                            if let Some((node_index, tab_index)) = self.tree.find_tab(space_view_id)
                            {
                                self.tree.set_active_tab(node_index, tab_index);
                            }
                        }
                    })
                    .body(|ui| {
                        if let Some(tree) = obj_tree.subtree(&space_view.space_path) {
                            show_children(ui, tree);
                        } else {
                            todo!()
                        }
                    });
                }
            });
    }
}

fn show_children(ui: &mut egui::Ui, tree: &ObjectTree) {
    for (path_comp, child) in &tree.children {
        if child.is_leaf() {
            ui.label(path_comp.to_string());
        } else {
            ui.collapsing(path_comp.to_string(), |ui| {
                show_children(ui, child);
            });
        }
    }
}

// ----------------------------------------------------------------------------

#[derive(serde::Deserialize, serde::Serialize)]
struct SpaceView {
    name: String,
    space_path: ObjPath,
    view_state: ViewState,
}

impl SpaceView {
    pub fn ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        space_info: &SpaceInfo,
        ui: &mut egui::Ui,
    ) -> egui::Response {
        crate::profile_function!(self.name.as_str());

        let mut time_objects = Objects::default();

        {
            crate::profile_scope!("time_query");
            let timeline = ctx.rec_cfg.time_ctrl.timeline();
            if let Some(timeline_store) = ctx.log_db.obj_db.store.get(timeline) {
                if let Some(time_query) = ctx.rec_cfg.time_ctrl.time_query() {
                    for obj_path in &space_info.objects {
                        if let Some(obj_store) = timeline_store.get(obj_path) {
                            if let Some(obj_type) =
                                ctx.log_db.obj_db.types.get(obj_path.obj_type_path())
                            {
                                if !is_sticky_type(obj_type) {
                                    time_objects.query_object(
                                        obj_store,
                                        &time_query,
                                        obj_path,
                                        obj_type,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut sticky_objects = Objects::default();
        {
            crate::profile_scope!("sticky_query");
            let timeline = ctx.rec_cfg.time_ctrl.timeline();
            if let Some(timeline_store) = ctx.log_db.obj_db.store.get(timeline) {
                for obj_path in &space_info.objects {
                    if let Some(obj_store) = timeline_store.get(obj_path) {
                        if let Some(obj_type) =
                            ctx.log_db.obj_db.types.get(obj_path.obj_type_path())
                        {
                            if is_sticky_type(obj_type) {
                                sticky_objects.query_object(
                                    obj_store,
                                    &TimeQuery::EVERYTHING,
                                    obj_path,
                                    obj_type,
                                );
                            }
                        }
                    }
                }
            }
        }

        self.view_state
            .ui(ctx, ui, &self.space_path, &time_objects, &sticky_objects)
    }
}

fn is_sticky_type(obj_type: &ObjectType) -> bool {
    obj_type == &ObjectType::TextEntry
}

// ----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct ViewState {
    // per space
    state_2d: crate::view2d::State2D,

    #[cfg(feature = "glow")]
    state_3d: crate::view3d::State3D,

    state_tensor: Option<crate::view_tensor::TensorViewState>,

    state_text_entry: crate::text_entry_view::TextEntryState,
}

impl ViewState {
    pub fn ui(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        ui: &mut egui::Ui,
        space: &ObjPath,
        time_objects: &Objects<'_>,
        sticky_objects: &Objects<'_>,
    ) -> egui::Response {
        crate::profile_function!();

        let has_2d = time_objects.has_any_2d();
        let has_3d = time_objects.has_any_3d();
        let multidim_tensor = multidim_tensor(time_objects);
        let has_text = sticky_objects.has_any_text_entries();

        let num_categories =
            has_2d as u32 + has_3d as u32 + multidim_tensor.is_some() as u32 + has_text as u32;

        match num_categories {
            0 => ui.label("(empty)"),
            1 => {
                if has_2d {
                    self.ui_2d(ctx, ui, space, time_objects)
                } else if has_3d {
                    self.ui_3d(ctx, ui, space, time_objects)
                } else if let Some(multidim_tensor) = multidim_tensor {
                    self.ui_tensor(ui, multidim_tensor)
                } else {
                    self.ui_text(ctx, ui, sticky_objects)
                }
            }
            _ => {
                todo!("Support mixing types by showing tabs for the different types")
            }
        }
    }

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
        objects: &Objects<'_>,
    ) -> egui::Response {
        #[cfg(feature = "glow")]
        return ui
            .vertical(|ui| {
                crate::view3d::view_3d(ctx, ui, &mut self.state_3d, Some(space), objects);
            })
            .response;

        #[cfg(not(feature = "glow"))]
        return ui.label(
            egui::RichText::new(
                "3D view not available (Rerun was compiled without the 'glow' feature)",
            )
            .size(24.0)
            .color(ui.visuals().warn_fg_color),
        );
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

// ----------------------------------------------------------------------------

struct TabViewer<'a, 'b> {
    ctx: &'a mut ViewerContext<'b>,
    spaces_info: &'a SpacesInfo,
    space_views: &'a mut HashMap<SpaceViewId, SpaceView>,
    hovered_space: Option<ObjPath>,
    maximized: &'a mut Option<SpaceViewId>,
}

impl<'a, 'b> egui_dock::TabViewer for TabViewer<'a, 'b> {
    type Tab = SpaceViewId;

    fn ui(&mut self, ui: &mut egui::Ui, space_view_id: &mut Self::Tab) {
        crate::profile_function!();

        ui.horizontal_top(|ui| {
            if ui.button("🗖").on_hover_text("Maximize space").clicked() {
                *self.maximized = Some(*space_view_id);
            }

            let space_view = self
                .space_views
                .get_mut(space_view_id)
                .expect("Should have been populated beforehand");

            if let Some(space_info) = self.spaces_info.spaces.get(&space_view.space_path) {
                let response = space_view.ui(self.ctx, space_info, ui);

                if response.hovered() {
                    self.hovered_space = Some(space_view.space_path.clone());
                }
            } else {
                ui.label("[Missing space]"); // TODO
            }
        });
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        let space_view = self
            .space_views
            .get_mut(tab)
            .expect("Should have been populated beforehand");
        space_view.name.clone().into()
    }
}

// ----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
pub(crate) struct ExperimentalViewportPanel {
    blueprint: Blueprint,
}

impl ExperimentalViewportPanel {
    pub fn ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        crate::profile_function!();

        let spaces_info = SpacesInfo::new(&ctx.log_db.obj_db, ctx.rec_cfg.time_ctrl.timeline());

        if ui.button("Reset space views / blueprint").clicked()
            || self.blueprint.space_views.is_empty()
        {
            self.blueprint = Blueprint::new(&spaces_info, ui.available_size());
        }

        egui::SidePanel::left("blueprint_panel")
            .resizable(true)
            .default_width(350.0)
            .show_inside(ui, |ui| {
                self.blueprint.tree_ui(ui, &ctx.log_db.obj_db.tree);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            let mut dock_style = egui_dock::Style::from_egui(ui.style().as_ref());
            dock_style.separator_width = 2.0;
            dock_style.show_close_buttons = false;
            dock_style.tab_include_scrollarea = false;

            let mut tab_viewer = TabViewer {
                ctx,
                spaces_info: &spaces_info,
                space_views: &mut self.blueprint.space_views,
                hovered_space: None,
                maximized: &mut self.blueprint.maximized,
            };

            egui_dock::DockArea::new(&mut self.blueprint.tree)
                .style(dock_style)
                .show_inside(ui, &mut tab_viewer);
        });
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct SpaceMakeInfo {
    id: SpaceViewId,
    path: ObjPath,
    size2d: Option<egui::Vec2>,
}

impl SpaceMakeInfo {
    fn is_2d(&self) -> bool {
        self.size2d.is_some()
    }
}

enum LayoutSplit {
    LeftRight(Box<LayoutSplit>, f32, Box<LayoutSplit>),
    TopBottom(Box<LayoutSplit>, f32, Box<LayoutSplit>),
    Leaf(SpaceMakeInfo),
}

fn tree_from_split(
    tree: &mut egui_dock::Tree<SpaceViewId>,
    parent: egui_dock::NodeIndex,
    split: &LayoutSplit,
) {
    match split {
        LayoutSplit::LeftRight(left, fraction, right) => {
            let [left_ni, right_ni] = tree.split_right(parent, *fraction, vec![]);
            tree_from_split(tree, left_ni, left);
            tree_from_split(tree, right_ni, right);
        }
        LayoutSplit::TopBottom(top, fraction, bottom) => {
            let [top_ni, bottom_ni] = tree.split_below(parent, *fraction, vec![]);
            tree_from_split(tree, top_ni, top);
            tree_from_split(tree, bottom_ni, bottom);
        }
        LayoutSplit::Leaf(space_info) => {
            tree.set_focused_node(parent);
            tree.push_to_focused_leaf(space_info.id);
        }
    }
}

// TODO(emilk): fix O(N^2) execution for layout_spaces
fn layout_spaces(size: egui::Vec2, spaces: &mut [SpaceMakeInfo]) -> LayoutSplit {
    assert!(!spaces.is_empty());

    if spaces.len() == 1 {
        LayoutSplit::Leaf(spaces[0].clone())
    } else {
        spaces.sort_by_key(|si| si.is_2d());
        let start_3d = spaces.partition_point(|si| !si.is_2d());

        if 0 < start_3d && start_3d < spaces.len() {
            split_spaces_at(size, spaces, start_3d)
        } else {
            // All 2D or all 3D
            let groups = group_by_path_prefix(spaces);
            assert!(groups.len() > 1);

            let num_spaces = spaces.len();

            let mut best_split = 0;
            let mut rearranged_spaces = vec![];
            for mut group in groups {
                rearranged_spaces.append(&mut group);

                let split_candidate = rearranged_spaces.len();
                if (split_candidate as f32 / num_spaces as f32 - 0.5).abs()
                    < (best_split as f32 / num_spaces as f32 - 0.5).abs()
                {
                    best_split = split_candidate;
                }
            }
            assert_eq!(rearranged_spaces.len(), num_spaces);
            assert!(0 < best_split && best_split < num_spaces,);

            split_spaces_at(size, &mut rearranged_spaces, best_split)
        }
    }
}

fn split_spaces_at(size: egui::Vec2, spaces: &mut [SpaceMakeInfo], index: usize) -> LayoutSplit {
    use egui::vec2;

    assert!(0 < index && index < spaces.len());

    let t = index as f32 / spaces.len() as f32;
    let desired_aspect_ratio = desired_aspect_ratio(spaces).unwrap_or(16.0 / 9.0);

    if size.x > desired_aspect_ratio * size.y {
        let left = layout_spaces(vec2(size.x * t, size.y), &mut spaces[..index]);
        let right = layout_spaces(vec2(size.x * (1.0 - t), size.y), &mut spaces[index..]);
        LayoutSplit::LeftRight(left.into(), t, right.into())
    } else {
        let top = layout_spaces(vec2(size.y, size.y * t), &mut spaces[..index]);
        let bottom = layout_spaces(vec2(size.y, size.x * (1.0 - t)), &mut spaces[index..]);
        LayoutSplit::TopBottom(top.into(), t, bottom.into())
    }
}

fn desired_aspect_ratio(spaces: &[SpaceMakeInfo]) -> Option<f32> {
    let mut sum = 0.0;
    let mut num = 0.0;
    for space in spaces {
        if let Some(size) = space.size2d {
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

fn group_by_path_prefix(space_infos: &[SpaceMakeInfo]) -> Vec<Vec<SpaceMakeInfo>> {
    if space_infos.len() < 2 {
        return vec![space_infos.to_vec()];
    }
    crate::profile_function!();

    let paths = space_infos
        .iter()
        .map(|space_info| space_info.path.to_components())
        .collect_vec();

    for i in 0.. {
        let mut groups: std::collections::BTreeMap<Option<&ObjPathComp>, Vec<&SpaceMakeInfo>> =
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
