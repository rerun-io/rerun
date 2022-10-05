use egui::{vec2, Vec2};
use itertools::Itertools as _;

use re_data_store::ObjectsBySpace;
use re_log_types::*;

use crate::{misc::HoveredSpace, ui::text_entry_view::TextEntryFetcher, ViewerContext};

// ----------------------------------------------------------------------------

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
enum SelectedSpace {
    All,
    /// None is the catch-all space for object without a space.
    Specific(Option<ObjPath>),
}

impl Default for SelectedSpace {
    fn default() -> Self {
        SelectedSpace::All
    }
}
// ----------------------------------------------------------------------------

#[derive(Clone)]
struct SpaceInfo {
    /// Path to the space.
    ///
    /// `None`: catch-all for all objects with no space assigned.
    space_path: Option<ObjPath>,

    /// Only set for 2D spaces
    size2d: Option<Vec2>,
}

impl SpaceInfo {
    fn obj_path_components(&self) -> Vec<ObjPathComp> {
        self.space_path
            .as_ref()
            .map(|space_path| space_path.to_components())
            .unwrap_or_default()
    }

    fn is_2d(&self) -> bool {
        self.size2d.is_some()
    }
}

// ----------------------------------------------------------------------------

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Tab {
    space: Option<ObjPath>,
}

struct TabViewer<'a, 'b> {
    ctx: &'a mut ViewerContext<'b>,
    objects: ObjectsBySpace<'b>,
    space_states: &'a mut SpaceStates,
    hovered_space: Option<ObjPath>,
    maximized: &'a mut Option<Tab>,
}

impl<'a, 'b> egui_dock::TabViewer for TabViewer<'a, 'b> {
    type Tab = Tab;

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        ui.horizontal_top(|ui| {
            if ui.button("🗖").on_hover_text("Maximize space").clicked() {
                *self.maximized = Some(tab.clone());
            }

            // Text entries work a bit different because they are completely isolated from
            // the main timeline selection: we do not filter them, rather we always want the
            // complete timeline!
            if let Some(objects) = self.objects.get(&tab.space.as_ref()) {
                if objects.has_any_text_entries() {
                    let state_log_messages =
                        TextEntryFetcher::from_context(self.ctx, tab.space.as_ref());
                    if !state_log_messages.is_empty() {
                        let response = state_log_messages.show(ui, self.ctx);
                        if response.hovered() {
                            self.hovered_space = tab.space.clone();
                        }
                    }

                    return;
                }
            }

            let hovered =
                self.space_states
                    .show_space(self.ctx, &self.objects, tab.space.as_ref(), ui);

            if hovered {
                self.hovered_space = tab.space.clone();
            }
        });
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        space_name(tab.space.as_ref()).into()
    }
}

// ----------------------------------------------------------------------------

/// A view of several spaces, organized to the users liking.
#[derive(Default, serde::Deserialize, serde::Serialize)]
struct View {
    // per space
    space_states: SpaceStates,

    tree: egui_dock::Tree<Tab>,

    /// Show one tab as maximized?
    maximized: Option<Tab>,
}

impl View {
    /// All spaces get their own tab, viewing one space at a time.
    #[allow(unused)]
    pub fn focus(all_spaces: &[Option<&ObjPath>]) -> Self {
        let tabs = all_spaces
            .iter()
            .map(|space| Tab {
                space: space.cloned(),
            })
            .collect_vec();

        Self::from_tree(egui_dock::Tree::new(tabs))
    }

    /// Show all spaces at the same time, in a tilemap.
    pub fn overview(available_size: Vec2, all_spaces: &[Option<&ObjPath>]) -> Self {
        let mut spaces = all_spaces
            .iter()
            .map(|opt_space_path| {
                let size2d = None; // TODO(emilk): estimate view sizes so we can do better auto-layouts.
                SpaceInfo {
                    space_path: opt_space_path.cloned(),
                    size2d,
                }
            })
            .collect_vec();

        let split = layout_spaces(available_size, &mut spaces);

        let mut tree = egui_dock::Tree::new(vec![]);
        tree_from_split(&mut tree, egui_dock::NodeIndex(0), &split);

        Self::from_tree(tree)
    }

    fn from_tree(tree: egui_dock::Tree<Tab>) -> Self {
        Self {
            tree,
            space_states: Default::default(),
            maximized: None,
        }
    }

    pub fn ui<'a, 'b>(
        &mut self,
        ctx: &'a mut ViewerContext<'b>,
        objects: ObjectsBySpace<'b>,
        ui: &mut egui::Ui,
    ) {
        let num_tabs = num_tabs(&self.tree);

        if num_tabs == 0 {
            // nothing to show
        } else if num_tabs == 1 {
            let tab = first_tab(&self.tree).unwrap();

            ui.strong(space_name(tab.space.as_ref()));
            if tab.space.as_ref() != ctx.rec_cfg.hovered_space.space() {
                ctx.rec_cfg.hovered_space = HoveredSpace::None;
            }

            self.space_states
                .show_space(ctx, &objects, tab.space.as_ref(), ui);
        } else if let Some(tab) = self.maximized.clone() {
            ui.horizontal(|ui| {
                if ui
                    .button("⬅")
                    .on_hover_text("Restore - show all spaces")
                    .clicked()
                {
                    self.maximized = None;
                }
                ui.strong(space_name(tab.space.as_ref()));
            });

            if tab.space.as_ref() != ctx.rec_cfg.hovered_space.space() {
                ctx.rec_cfg.hovered_space = HoveredSpace::None;
            }

            self.space_states
                .show_space(ctx, &objects, tab.space.as_ref(), ui);
        } else {
            let mut tab_viewer = TabViewer {
                ctx,
                objects,
                space_states: &mut self.space_states,
                hovered_space: None,
                maximized: &mut self.maximized,
            };

            let dock_style = egui_dock::Style {
                separator_width: 2.0,
                show_close_buttons: false,
                ..egui_dock::Style::from_egui(ui.style().as_ref())
            };

            // TODO(emilk): fix egui_dock: this scope shouldn't be needed
            ui.scope(|ui| {
                egui_dock::DockArea::new(&mut self.tree)
                    .style(dock_style)
                    .show_inside(ui, &mut tab_viewer);
            });

            let hovered_space = tab_viewer.hovered_space;

            if hovered_space.as_ref() != ctx.rec_cfg.hovered_space.space() {
                ctx.rec_cfg.hovered_space = HoveredSpace::None;
            }
        }
    }
}

// TODO(emilk): move this into `egui_dock`
fn num_tabs(tree: &egui_dock::Tree<Tab>) -> usize {
    let mut count = 0;
    for node in tree.iter() {
        if let egui_dock::Node::Leaf { tabs, .. } = node {
            count += tabs.len();
        }
    }
    count
}

// TODO(emilk): move this into `egui_dock`
fn first_tab(tree: &egui_dock::Tree<Tab>) -> Option<Tab> {
    for node in tree.iter() {
        if let egui_dock::Node::Leaf { tabs, .. } = node {
            if let Some(first) = tabs.first() {
                return Some(first.clone());
            }
        }
    }
    None
}

// ----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct SpacesPanel {
    // In the future we will support multiple user-defined views,
    // but for now we only have one.
    view: View,
}

impl SpacesPanel {
    pub fn ui(&mut self, ctx: &mut ViewerContext<'_>, ui: &mut egui::Ui) {
        crate::profile_function!();

        if ctx.log_db.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.heading("No data");
            });
            return;
        }

        let objects = ctx
            .rec_cfg
            .time_ctrl
            .selected_objects(ctx.log_db)
            .partition_on_space();

        let all_spaces = {
            // `objects` contain all spaces that exist in this time,
            // but we want to show all spaces that could ever exist.
            // Othewise we get a lot of flicker of spaces as we play back data.
            let mut all_spaces = ctx.log_db.spaces().map(Some).collect_vec();
            if objects.contains_key(&None) {
                // Some objects lack a space, so they end up in the `None` space.
                // TODO(emilk): figure this out beforehand somehow.
                all_spaces.push(None);
            }
            all_spaces.sort_unstable();
            all_spaces
        };

        for space in &all_spaces {
            // Make sure the view has all spaces:
            let tab = Tab {
                space: space.cloned(),
            };
            if self.view.tree.find_tab(&tab).is_none() {
                self.view = View::overview(ui.available_size(), &all_spaces);
                break;
            }
        }

        self.view.ui(ctx, objects, ui);
    }
}

// ----------------------------------------------------------------------------

#[derive(Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub(crate) struct SpaceStates {
    // per space
    state_2d: ahash::HashMap<Option<ObjPath>, crate::view2d::State2D>,

    #[cfg(feature = "glow")]
    state_3d: ahash::HashMap<Option<ObjPath>, crate::view3d::State3D>,

    state_tensor: ahash::HashMap<Option<ObjPath>, crate::view_tensor::TensorViewState>,
}

impl SpaceStates {
    /// Returns `true` if hovered.
    fn show_space(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        objects: &ObjectsBySpace<'_>,
        space: Option<&ObjPath>,
        ui: &mut egui::Ui,
    ) -> bool {
        crate::profile_function!(space_name(space));

        let mut hovered = false;

        let objects = if let Some(objects) = objects.get(&space) {
            objects
        } else {
            return hovered;
        };

        let objects = objects.filter(|props| {
            ctx.rec_cfg
                .projected_object_properties
                .get(props.obj_path)
                .visible
        });

        // We have a special tensor viewer that only works
        // when we only have a single tensor (and no bounding boxes etc).
        // It is also not as great for images as the nomral 2d view (at least not yet).
        // This is a hacky-way of detecting this special case.
        // TODO(emilk): integrate the tensor viewer into the 2D viewer instead,
        // so we can stack bounding boxes etc on top of it.
        if objects.image.len() == 1 {
            let image = objects.image.first().unwrap().1;
            let tensor = image.tensor;

            // Ignore tensors that likely represent images.
            if tensor.num_dim() > 3
                || tensor.num_dim() == 3 && tensor.shape.last().unwrap().size > 4
            {
                let state_tensor = self
                    .state_tensor
                    .entry(space.cloned())
                    .or_insert_with(|| crate::ui::view_tensor::TensorViewState::create(tensor));

                hovered |= ui
                    .vertical(|ui| {
                        crate::view_tensor::view_tensor(ui, state_tensor, tensor);
                    })
                    .response
                    .hovered;

                return hovered;
            }
        }

        let num_cats = objects.has_any_2d() as u32
            + objects.has_any_3d() as u32
            // NOTE: cannot actually happen at the moment as text entries are caught early..
            // better safe than sorry.
            + objects.has_any_text_entries() as u32;
        if num_cats > 1 {
            re_log::warn_once!(
                "Space {:?} contains multiple categories of objects \
                    (e.g. both 2D and 3D, both 2D and text entries, etc...)",
                space_name(space)
            );
        }

        if objects.has_any_2d() {
            let state_2d = self.state_2d.entry(space.cloned()).or_default();
            let response = crate::view2d::view_2d(ctx, ui, state_2d, space, &objects);
            hovered |= response.hovered();
        }

        if objects.has_any_3d() {
            #[cfg(feature = "glow")]
            ui.vertical(|ui| {
                let state_3d = self.state_3d.entry(space.cloned()).or_default();
                let response = crate::view3d::view_3d(ctx, ui, state_3d, space, &objects);
                hovered |= response.hovered();
            });

            #[cfg(not(feature = "glow"))]
            ui.label(
                egui::RichText::new(
                    "3D view not availble (Rerun was compiled without the 'glow' feature)",
                )
                .size(24.0)
                .color(ui.visuals().warn_fg_color),
            );
        }

        if !hovered && ctx.rec_cfg.hovered_space.space() == space {
            ctx.rec_cfg.hovered_space = HoveredSpace::None;
        }

        hovered
    }
}

fn space_name(space: Option<&ObjPath>) -> String {
    if let Some(space) = space {
        let name = space.to_string();
        if name == "/" {
            name
        } else {
            name.strip_prefix('/').unwrap_or(name.as_str()).to_owned()
        }
    } else {
        "<default space>".to_owned()
    }
}

// ----------------------------------------------------------------------------

enum LayoutSplit {
    LeftRight(Box<LayoutSplit>, f32, Box<LayoutSplit>),
    TopBottom(Box<LayoutSplit>, f32, Box<LayoutSplit>),
    Leaf(SpaceInfo),
}

fn tree_from_split(
    tree: &mut egui_dock::Tree<Tab>,
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
            let tab = Tab {
                space: space_info.space_path.clone(),
            };
            tree.set_focused_node(parent);
            tree.push_to_focused_leaf(tab);
        }
    }
}

// TODO(emilk): fix O(N^2) execution for layout_spaces
fn layout_spaces(size: Vec2, spaces: &mut [SpaceInfo]) -> LayoutSplit {
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

fn split_spaces_at(size: Vec2, spaces: &mut [SpaceInfo], index: usize) -> LayoutSplit {
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

fn desired_aspect_ratio(spaces: &[SpaceInfo]) -> Option<f32> {
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

fn group_by_path_prefix(space_infos: &[SpaceInfo]) -> Vec<Vec<SpaceInfo>> {
    if space_infos.len() < 2 {
        return vec![space_infos.to_vec()];
    }
    crate::profile_function!();

    let paths = space_infos
        .iter()
        .map(|space_info| space_info.obj_path_components())
        .collect_vec();

    for i in 0.. {
        let mut groups: std::collections::BTreeMap<Option<&ObjPathComp>, Vec<&SpaceInfo>> =
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
