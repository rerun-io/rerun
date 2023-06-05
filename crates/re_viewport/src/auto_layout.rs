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

use std::collections::BTreeMap;

use ahash::HashMap;
use egui::Vec2;
use itertools::Itertools as _;

use re_data_store::{EntityPath, EntityPathPart};
use re_space_view_spatial::SpatialNavigationMode;
use re_viewer_context::{SpaceViewId, ViewerContext};

use super::{
    space_view::{SpaceViewBlueprint, SpaceViewState},
    view_category::ViewCategory,
};

#[derive(Clone, Debug)]
pub struct SpaceMakeInfo {
    pub id: SpaceViewId,

    /// Some path we use to group the views by
    pub path: EntityPath,

    pub category: ViewCategory,

    /// Desired aspect ratio, if any.
    pub aspect_ratio: Option<f32>,
}

enum LayoutSplit {
    LeftRight(Box<LayoutSplit>, f32, Box<LayoutSplit>),
    TopBottom(Box<LayoutSplit>, f32, Box<LayoutSplit>),
    Leaf(SpaceMakeInfo),
}

enum SplitDirection {
    LeftRight { left: Vec2, t: f32, right: Vec2 },
    TopBottom { top: Vec2, t: f32, bottom: Vec2 },
}

pub(crate) fn tree_from_space_views(
    ctx: &mut ViewerContext<'_>,
    viewport_size: egui::Vec2,
    visible: &std::collections::BTreeSet<SpaceViewId>,
    space_views: &HashMap<SpaceViewId, SpaceViewBlueprint>,
    space_view_states: &HashMap<SpaceViewId, SpaceViewState>,
) -> egui_tiles::Tree<SpaceViewId> {
    let mut space_make_infos = space_views
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
            let aspect_ratio = space_view_states.get(space_view_id).and_then(|state| {
                ctx.space_view_class_registry
                    .query(space_view.class)
                    .ok()
                    .and_then(|class| class.preferred_tile_aspect_ratio(state.state.as_ref()))
            });

            SpaceMakeInfo {
                id: *space_view_id,
                path: space_view.space_path.clone(),
                category: space_view.category,
                aspect_ratio,
            }
        })
        .collect_vec();

    if space_make_infos.is_empty() {
        egui_tiles::Tree::empty()
    } else {
        let mut tiles = egui_tiles::Tiles::default();
        // Users often organize by path prefix, so we start by splitting along that
        let layout = layout_by_path_prefix(viewport_size, &mut space_make_infos);
        let root = tree_from_split(&mut tiles, &layout);
        egui_tiles::Tree::new(root, tiles)
    }
}

fn tree_from_split(
    tiles: &mut egui_tiles::Tiles<SpaceViewId>,
    split: &LayoutSplit,
) -> egui_tiles::TileId {
    match split {
        LayoutSplit::LeftRight(left, fraction, right) => {
            let container = egui_tiles::Linear::new_binary(
                egui_tiles::LinearDir::Horizontal,
                [tree_from_split(tiles, left), tree_from_split(tiles, right)],
                *fraction,
            );
            tiles.insert_container(container)
        }
        LayoutSplit::TopBottom(top, fraction, bottom) => {
            let container = egui_tiles::Linear::new_binary(
                egui_tiles::LinearDir::Vertical,
                [tree_from_split(tiles, top), tree_from_split(tiles, bottom)],
                *fraction,
            );
            tiles.insert_container(container)
        }
        LayoutSplit::Leaf(space_info) => tiles.insert_pane(space_info.id),
    }
}

/// Group categories together, i.e. so that 2D stuff is next to 2D stuff, and text logs are next to text logs.
fn layout_by_category(viewport_size: egui::Vec2, spaces: &mut [SpaceMakeInfo]) -> LayoutSplit {
    assert!(!spaces.is_empty());

    if spaces.len() == 1 {
        LayoutSplit::Leaf(spaces[0].clone())
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
        LayoutSplit::Leaf(spaces[0].clone())
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
        groups.entry(info.category).or_default().push(info.clone());
    }
    groups.into_values().collect()
}

fn group_by_path_prefix(space_infos: &[SpaceMakeInfo]) -> Vec<Vec<SpaceMakeInfo>> {
    if space_infos.len() < 2 {
        return vec![space_infos.to_vec()];
    }
    re_tracing::profile_function!();

    let paths = space_infos
        .iter()
        .map(|space_info| space_info.path.as_slice().to_vec())
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
