//! Code for automatic layout of space views.
//!
//! This uses rough heuristics and have a lot of room for improvement.
//!
//! Some of the heuristics include:
//! * We want similar space views together. Similar can mean:
//!   * Same category (2D vs text vs …)
//!   * Similar entity path (common prefix)
//! * We also want to pick aspect ratios that fit the data pretty well
// TODO(emilk): fix O(N^2) execution time (where N = number of spaces)

use core::panic;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ahash::HashMap;
use egui::Vec2;
use egui_dock::NodeIndex;
use itertools::Itertools as _;

use lazy_static::lazy_static;
use re_data_store::{EntityPath, EntityPathPart};

use crate::depthai::depthai;

use super::{
    space_view::{SpaceView, SpaceViewKind},
    view_category::ViewCategory,
    viewport::Tab,
    SpaceViewId,
};

#[derive(Clone, Debug)]
pub struct SpaceMakeInfo {
    pub id: SpaceViewId,

    /// Some path we use to group the views by
    pub path: Option<EntityPath>,

    pub category: Option<ViewCategory>,

    /// Desired aspect ratio, if any.
    pub aspect_ratio: Option<f32>,

    pub kind: SpaceViewKind,
}

enum LayoutSplit {
    LeftRight(Box<LayoutSplit>, f32, Box<LayoutSplit>),
    TopBottom(Box<LayoutSplit>, f32, Box<LayoutSplit>),
    Leaf(Vec<SpaceMakeInfo>),
}

impl std::fmt::Debug for LayoutSplit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutSplit::LeftRight(left, fraction, right) => {
                write!(f, "LeftRight({:?}, {}, {:?})", left, fraction, right)
            }
            LayoutSplit::TopBottom(top, fraction, bottom) => {
                write!(f, "TopBottom({:?}, {}, {:?})", top, fraction, bottom)
            }
            LayoutSplit::Leaf(spaces) => {
                write!(
                    f,
                    "Leaf({:?})",
                    spaces.iter().map(|s| s.path.clone()).collect_vec()
                )
            }
        }
    }
}

enum SplitDirection {
    LeftRight { left: Vec2, t: f32, right: Vec2 },
    TopBottom { top: Vec2, t: f32, bottom: Vec2 },
}

fn right_panel_split() -> LayoutSplit {
    LayoutSplit::TopBottom(
        LayoutSplit::Leaf(vec![CONFIG_SPACE_VIEW.clone(), STATS_SPACE_VIEW.clone()]).into(),
        0.7,
        LayoutSplit::Leaf(vec![SELECTION_SPACE_VIEW.clone()]).into(),
    )
}

// Creates space make infos for constant space views.
// This is needed to be able to search for these views in the tree later, based on the SpaceViewId
lazy_static! {
    static ref CONFIG_SPACE_VIEW: SpaceMakeInfo = SpaceMakeInfo {
        id: SpaceViewId::random(),
        path: None,
        category: None,
        aspect_ratio: None,
        kind: SpaceViewKind::Config,
    };
    static ref STATS_SPACE_VIEW: SpaceMakeInfo = SpaceMakeInfo {
        id: SpaceViewId::random(),
        path: None,
        category: None,
        aspect_ratio: None,
        kind: SpaceViewKind::Stats,
    };
    static ref SELECTION_SPACE_VIEW: SpaceMakeInfo = SpaceMakeInfo {
        id: SpaceViewId::random(),
        path: None,
        category: None,
        aspect_ratio: None,
        kind: SpaceViewKind::Selection,
    };
    static ref CONSTANT_SPACE_VIEWS: Vec<SpaceViewId> = vec![
        CONFIG_SPACE_VIEW.id,
        STATS_SPACE_VIEW.id,
        SELECTION_SPACE_VIEW.id,
    ];
}

fn push_space_view_to_leaf(
    tree: &mut egui_dock::Tree<Tab>,
    leaf: NodeIndex,
    space_view: &SpaceView,
) {
    tree.set_focused_node(leaf);
    tree.push_to_focused_leaf(space_view.into());
}

fn find_space_path_in_tree(
    tree: &egui_dock::Tree<Tab>,
    space_view_path: &EntityPath,
) -> Option<Tab> {
    tree.tabs()
        .find(|tab| {
            let Some(path) = &tab.space_path else {
            return false;
        };
            path == space_view_path
        })
        .cloned()
}

fn find_top_left_leaf(tree: &egui_dock::Tree<Tab>) -> NodeIndex {
    let mut node = NodeIndex::root();
    loop {
        if tree[node].is_leaf() {
            println!("Node: {node:?}");
            return node;
        }
        node = node.right();
    }
}

// /// Layout `CAM_B` `CAM_A` | `CAM_C` with 3d views on top and 2d views on the bottom.
// fn create_inner_viewport_layout(spaces: &Vec<SpaceMakeInfo>) -> LayoutSplit {
//     let mut groups: HashMap<EntityPathPart, (Vec<SpaceMakeInfo>, Vec<SpaceMakeInfo>)> =
//         HashMap::default();

//     for space in spaces {
//         if let Some(path) = &space.path {
//             let base_path = match path.as_slice().first() {
//                 Some(part) => part.clone(),
//                 None => continue,
//             };

//             let (views_2d, views_3d) = groups.entry(base_path).or_default();

//             if path.len() > 1 {
//                 views_2d.push(space.clone());
//             } else {
//                 views_3d.push(space.clone());
//             }
//         }
//     }

//     let mut sorted_groups: BTreeMap<EntityPathPart, (Vec<SpaceMakeInfo>, Vec<SpaceMakeInfo>)> =
//         BTreeMap::new();
//     for (key, value) in groups {
//         sorted_groups.insert(key, value);
//     }

//     let mut layouts: VecDeque<LayoutSplit> = VecDeque::new();

//     for (_base_path, (views_2d, views_3d)) in sorted_groups {
//         let layout_2d = LayoutSplit::Leaf(views_2d);
//         let layout_3d = LayoutSplit::Leaf(views_3d);

//         layouts.push_back(LayoutSplit::TopBottom(
//             Box::new(layout_3d),
//             0.5,
//             Box::new(layout_2d),
//         ));
//     }
//     if layouts.len() > 1 {
//         create_horizontal_layout(&mut layouts).1
//     } else {
//         LayoutSplit::Leaf(spaces.clone())
//     }
// }

// fn create_horizontal_layout(vertical_splits: &mut VecDeque<LayoutSplit>) -> (f32, LayoutSplit) {
//     if vertical_splits.len() == 1 {
//         return (1.0, vertical_splits.pop_front().unwrap());
//     }
//     let left = vertical_splits.pop_front().unwrap();
//     let (mut n_splits, mut right) = create_horizontal_layout(vertical_splits);
//     n_splits += 1.0;
//     right = LayoutSplit::LeftRight(Box::new(left), 1.0 / n_splits, Box::new(right));
//     (n_splits, right)
// }

/// Layout `CAM_A` `CAM_B` | `CAM_C` with 3d views on top and 2d views on the bottom in the same group. (only one 2d and one 3d view visible from the start)
fn create_inner_viewport_layout(spaces: &Vec<SpaceMakeInfo>) -> LayoutSplit {
    let mut groups: HashMap<EntityPathPart, (Vec<SpaceMakeInfo>, Vec<SpaceMakeInfo>)> =
        HashMap::default();

    for space in spaces {
        if let Some(path) = &space.path {
            let base_path = match path.as_slice().first() {
                Some(part) => part.clone(),
                None => continue,
            };

            let (views_2d, views_3d) = groups.entry(base_path).or_default();

            if path.len() > 1 {
                views_2d.push(space.clone());
            } else {
                views_3d.push(space.clone());
            }
        }
    }

    let mut sorted_groups: BTreeMap<EntityPathPart, (Vec<SpaceMakeInfo>, Vec<SpaceMakeInfo>)> =
        BTreeMap::new();
    for (key, value) in groups {
        sorted_groups.insert(key, value);
    }
    let mut all_2d = Vec::new();
    let mut all_3d = Vec::new();
    for (_base_path, (views_2d, views_3d)) in sorted_groups {
        all_2d.extend(views_2d);
        all_3d.extend(views_3d);
    }
    LayoutSplit::TopBottom(
        LayoutSplit::Leaf(all_3d).into(),
        0.5,
        LayoutSplit::Leaf(all_2d).into(),
    )
}

/// Default layout of space views tuned for depthai-viewer
pub(crate) fn default_tree_from_space_views(
    viewport_size: egui::Vec2,
    visible: &std::collections::BTreeSet<SpaceViewId>,
    space_views: &HashMap<SpaceViewId, SpaceView>,
    is_maximized: bool,
) -> egui_dock::Tree<Tab> {
    // TODO(filip): Implement sensible auto layout when space views changes.
    // Something like:
    // - Get the tabs that need to be added or removed
    // - Removal is easy, just remove the tab
    // - Addition should try to layout like currently 3d, 2d views. New views just appear in the top left corner i guess.
    let mut tree = egui_dock::Tree::new(Vec::new());

    let spaces = space_views
        .iter()
        .filter(|(space_view_id, _space_view)| visible.contains(space_view_id))
        // Sort for determinism:
        .sorted_by_key(|(space_view_id, space_view)| {
            (
                &space_view.space_path,
                &space_view.display_name,
                *space_view_id,
            )
        })
        .map(|(space_view_id, space_view)| {
            let aspect_ratio = match space_view.category {
                ViewCategory::Spatial => {
                    let state_spatial = &space_view.view_state.state_spatial;
                    match *state_spatial.nav_mode.get() {
                        // This is the only thing where the aspect ratio makes complete sense.
                        super::view_spatial::SpatialNavigationMode::TwoD => {
                            let size = state_spatial.scene_bbox_accum.size();
                            Some(size.x / size.y)
                        }
                        // 3D scenes can be pretty flexible
                        super::view_spatial::SpatialNavigationMode::ThreeD => None,
                    }
                }
                ViewCategory::Tensor | ViewCategory::TimeSeries => Some(1.0), // Not sure if we should do `None` here.
                ViewCategory::Text | ViewCategory::NodeGraph => Some(2.0),    // Make text logs wide
                ViewCategory::BarChart => None,
            };

            SpaceMakeInfo {
                id: *space_view_id,
                path: Some(space_view.space_path.clone()),
                category: Some(space_view.category),
                aspect_ratio,
                kind: SpaceViewKind::Data,
            }
        })
        .collect_vec();

    if !spaces.is_empty() || is_maximized {
        let layout = {
            if is_maximized {
                let space_view_id = visible.first().unwrap();
                if space_views.get(space_view_id).is_none() {
                    if space_view_id == &STATS_SPACE_VIEW.id {
                        println!("Space view is stats space view!");
                        LayoutSplit::Leaf(vec![SpaceMakeInfo {
                            id: *space_view_id,
                            path: None,
                            category: None,
                            aspect_ratio: None,
                            kind: SpaceViewKind::Stats,
                        }])
                    } else {
                        panic!("Can't maximize this space view");
                    }
                } else {
                    LayoutSplit::Leaf(spaces)
                }
            } else {
                LayoutSplit::LeftRight(
                    create_inner_viewport_layout(&spaces).into(),
                    0.7,
                    right_panel_split().into(),
                )
            }
        };
        tree_from_split(&mut tree, NodeIndex::root(), &layout);
    } else {
        tree_from_split(
            &mut tree,
            NodeIndex::root(),
            &LayoutSplit::LeftRight(
                LayoutSplit::Leaf(vec![]).into(),
                0.7,
                right_panel_split().into(),
            ),
        );
    }
    if !is_maximized {
        // Always set the color cam (if available - currently the approach is really bad as I just check for CAM_A,
        // should be improved upon to search for camera name in connected_cameras) as the active tab and then the config tab as the active tab
        let tree_clone = tree.clone();
        let color_tabs = tree_clone.tabs().filter(|tab| {
            if let Some(space_path) = tab.space_path.clone() {
                if let Some(first_part) = space_path.as_slice().first() {
                    first_part == &EntityPathPart::from("CAM_A")
                } else {
                    false
                }
            } else {
                false
            }
        });
        for color_tab in color_tabs {
            let (node_index, tab_index) = tree.find_tab(color_tab).unwrap();
            tree.set_active_tab(node_index, tab_index);
        }
        let (config_node, config_tab) = tree
            .find_tab(
                tree.tabs()
                    .find(|tab| tab.space_view_id == CONFIG_SPACE_VIEW.id)
                    .unwrap(), // CONFIG_SPACE_VIEW is always present
            )
            .unwrap();
        tree.set_active_tab(config_node, config_tab);
    }
    tree
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
        LayoutSplit::Leaf(space_infos) => {
            tree.set_focused_node(parent);
            for space_info in space_infos {
                tree.push_to_focused_leaf(Tab {
                    space_view_id: space_info.id,
                    space_view_kind: space_info.kind,
                    space_path: space_info.path.clone(),
                });
            }
        }
    }
}

/// Group categories together, i.e. so that 2D stuff is next to 2D stuff, and text logs are next to text logs.
fn layout_by_category(viewport_size: egui::Vec2, spaces: &mut [SpaceMakeInfo]) -> LayoutSplit {
    assert!(!spaces.is_empty());

    if spaces.len() == 1 {
        LayoutSplit::Leaf(spaces.to_vec())
    } else {
        let groups = group_by_category(spaces);

        if groups.len() == 1 {
            // All same category
            layout_by_path_prefix(viewport_size, spaces)
        } else {
            // Mixed categories.
            split_groups(viewport_size, groups)
        }
    }
}

/// Put spaces with common path prefix close together.
fn layout_by_path_prefix(viewport_size: egui::Vec2, spaces: &mut [SpaceMakeInfo]) -> LayoutSplit {
    assert!(!spaces.is_empty());

    if spaces.len() == 1 {
        LayoutSplit::Leaf(spaces.to_vec())
    } else {
        let groups = group_by_path_prefix(spaces);

        if groups.len() == 1 {
            // failed to separate by group - try category instead:
            layout_by_category(viewport_size, spaces)
        } else {
            split_groups(viewport_size, groups)
        }
    }
}

fn split_groups(viewport_size: egui::Vec2, groups: Vec<Vec<SpaceMakeInfo>>) -> LayoutSplit {
    let (mut spaces, split_point) = find_group_split_point(groups);
    split_spaces_at(viewport_size, &mut spaces, split_point)
}

fn find_group_split_point(groups: Vec<Vec<SpaceMakeInfo>>) -> (Vec<SpaceMakeInfo>, usize) {
    assert!(groups.len() > 1);

    let num_spaces: usize = groups.iter().map(|g| g.len()).sum();

    let mut best_split = 0;
    let mut rearranged_spaces = vec![];
    for mut group in groups {
        rearranged_spaces.append(&mut group);

        let split_candidate = rearranged_spaces.len();

        // Prefer the split that is closest to the middle:
        if (split_candidate as f32 / num_spaces as f32 - 0.5).abs()
            < (best_split as f32 / num_spaces as f32 - 0.5).abs()
        {
            best_split = split_candidate;
        }
    }
    assert_eq!(rearranged_spaces.len(), num_spaces);
    assert!(0 < best_split && best_split < num_spaces,);

    (rearranged_spaces, best_split)
}

fn suggest_split_direction(
    viewport_size: egui::Vec2,
    spaces: &[SpaceMakeInfo],
    split_index: usize,
) -> SplitDirection {
    use egui::vec2;

    assert!(0 < split_index && split_index < spaces.len());

    let t = split_index as f32 / spaces.len() as f32;

    let desired_aspect_ratio = desired_aspect_ratio(spaces).unwrap_or(16.0 / 9.0);

    if viewport_size.x > desired_aspect_ratio * viewport_size.y {
        let left = vec2(viewport_size.x * t, viewport_size.y);
        let right = vec2(viewport_size.x * (1.0 - t), viewport_size.y);
        SplitDirection::LeftRight { left, t, right }
    } else {
        let top = vec2(viewport_size.x, viewport_size.y * t);
        let bottom = vec2(viewport_size.x, viewport_size.y * (1.0 - t));
        SplitDirection::TopBottom { top, t, bottom }
    }
}

fn split_spaces_at(
    viewport_size: egui::Vec2,
    spaces: &mut [SpaceMakeInfo],
    split_index: usize,
) -> LayoutSplit {
    assert!(0 < split_index && split_index < spaces.len());

    match suggest_split_direction(viewport_size, spaces, split_index) {
        SplitDirection::LeftRight { left, t, right } => {
            let left = layout_by_path_prefix(left, &mut spaces[..split_index]);
            let right = layout_by_path_prefix(right, &mut spaces[split_index..]);
            LayoutSplit::LeftRight(left.into(), t, right.into())
        }
        SplitDirection::TopBottom { top, t, bottom } => {
            let top = layout_by_path_prefix(top, &mut spaces[..split_index]);
            let bottom = layout_by_path_prefix(bottom, &mut spaces[split_index..]);
            LayoutSplit::TopBottom(top.into(), t, bottom.into())
        }
    }
}

/// If we need to pick only one aspect ratio for all these spaces, what is a good aspect ratio?
///
/// This is a very, VERY, rough heuristic. It really only work in a few cases:
///
/// * All spaces have similar aspect ration (e.g. all portrait or all landscape)
/// * Only one space care about aspect ratio, and the other are flexible
/// * A mix of the above
///
/// Still, it is better than nothing.
fn desired_aspect_ratio(spaces: &[SpaceMakeInfo]) -> Option<f32> {
    // Taking the arithmetic mean of all given aspect ratios.
    // It makes very little sense, unless the aspect ratios are all close already.
    // Perhaps a mode or median would make more sense?

    let mut sum = 0.0;
    let mut num = 0.0;
    for space in spaces {
        if let Some(aspect_ratio) = space.aspect_ratio {
            if aspect_ratio.is_finite() {
                sum += aspect_ratio;
                num += 1.0;
            }
        }
    }

    (num != 0.0).then_some(sum / num)
}

fn group_by_category(space_infos: &[SpaceMakeInfo]) -> Vec<Vec<SpaceMakeInfo>> {
    let mut groups: BTreeMap<ViewCategory, Vec<SpaceMakeInfo>> = Default::default();
    for info in space_infos {
        let Some(category) = info.category else {
            continue;
        };
        groups.entry(category).or_default().push(info.clone());
    }
    groups.into_values().collect()
}

fn group_by_path_prefix(space_infos: &[SpaceMakeInfo]) -> Vec<Vec<SpaceMakeInfo>> {
    if space_infos.len() < 2 {
        return vec![space_infos.to_vec()];
    }
    crate::profile_function!();

    let paths = space_infos
        .iter()
        .map(|space_info| {
            let Some(path) = &space_info.path else {
                panic!("Space {:?} has no path", space_info);
            };
            path.as_slice().to_vec()
        })
        .collect_vec();

    for i in 0.. {
        let mut groups: BTreeMap<Option<&EntityPathPart>, Vec<&SpaceMakeInfo>> = Default::default();
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
