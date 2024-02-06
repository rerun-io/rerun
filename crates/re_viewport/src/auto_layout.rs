//! Code for automatic layout of space views.
//!
//! This uses some very rough heuristics and have a lot of room for improvement.

use std::collections::BTreeMap;

use itertools::Itertools as _;

use re_viewer_context::{SpaceViewClassIdentifier, SpaceViewId};

use re_space_view::SpaceViewBlueprint;

#[derive(Clone, Debug)]
struct SpaceMakeInfo {
    id: SpaceViewId,
    class_identifier: SpaceViewClassIdentifier,
    layout_priority: re_viewer_context::SpaceViewClassLayoutPriority,
}

pub(crate) fn tree_from_space_views(
    space_view_class_registry: &re_viewer_context::SpaceViewClassRegistry,
    space_views: &BTreeMap<SpaceViewId, SpaceViewBlueprint>,
) -> egui_tiles::Tree<SpaceViewId> {
    re_log::trace!("Auto-layout of {} space views", space_views.len());

    if space_views.is_empty() {
        return egui_tiles::Tree::empty("viewport_tree");
    }

    let space_make_infos = space_views
        .iter()
        // Sort for determinism:
        .sorted_by_key(|(space_view_id, space_view)| {
            (
                &space_view.space_origin,
                &space_view.display_name,
                *space_view_id,
            )
        })
        .map(|(space_view_id, space_view)| {
            let class_identifier = *space_view.class_identifier();
            let layout_priority = space_view
                .class(space_view_class_registry)
                .layout_priority();
            SpaceMakeInfo {
                id: *space_view_id,
                class_identifier,
                layout_priority,
            }
        })
        .collect_vec();

    let mut tiles = egui_tiles::Tiles::default();

    let root = if space_make_infos.len() == 1 {
        tiles.insert_pane(space_make_infos[0].id)
    } else if space_make_infos.len() == 3 {
        // Special-case for common case that doesn't fit nicely in a grid
        arrange_three(
            [
                space_make_infos[0].clone(),
                space_make_infos[1].clone(),
                space_make_infos[2].clone(),
            ],
            &mut tiles,
        )
    } else if space_make_infos.len() <= 12 {
        // Arrange it all in a grid that is responsive to changes in viewport size:
        let child_tile_ids = space_make_infos
            .into_iter()
            .map(|smi| tiles.insert_pane(smi.id))
            .collect_vec();
        tiles.insert_grid_tile(child_tile_ids)
    } else {
        // So many space views - lets group by class and put the members of each group into tabs:
        let mut grouped_by_class: BTreeMap<SpaceViewClassIdentifier, Vec<SpaceMakeInfo>> =
            Default::default();
        for smi in space_make_infos {
            grouped_by_class
                .entry(smi.class_identifier)
                .or_default()
                .push(smi);
        }

        let groups = grouped_by_class
            .values()
            .cloned()
            .sorted_by_key(|group| -(group[0].layout_priority as isize));

        let tabs = groups
            .into_iter()
            .map(|group| {
                let children = group
                    .into_iter()
                    .map(|smi| tiles.insert_pane(smi.id))
                    .collect_vec();
                tiles.insert_tab_tile(children)
            })
            .collect_vec();

        if tabs.len() == 1 {
            tabs[0]
        } else {
            tiles.insert_grid_tile(tabs)
        }
    };

    egui_tiles::Tree::new("viewport_tree", root, tiles)
}

fn arrange_three(
    mut spaces: [SpaceMakeInfo; 3],
    tiles: &mut egui_tiles::Tiles<SpaceViewId>,
) -> egui_tiles::TileId {
    // We will arrange it like so:
    //
    // +-------------+
    // |             |
    // |             |
    // |             |
    // +-------+-----+
    // |       |     |
    // |       |     |
    // +-------+-----+
    //
    // or like so:
    //
    // +-----------------------+
    // |          |            |
    // |          |            |
    // |          +------------+
    // |          |            |
    // |          |            |
    // |          |            |
    // +----------+------------+
    //
    // But which space gets a full side, and which doesn't?
    // Answer: we prioritize them based on a class-specific layout priority:

    spaces.sort_by_key(|smi| -(smi.layout_priority as isize));

    let pane_ids = spaces
        .into_iter()
        .map(|smi| tiles.insert_pane(smi.id))
        .collect_vec();

    let inner_grid = tiles.insert_grid_tile(vec![pane_ids[1], pane_ids[2]]);
    tiles.insert_grid_tile(vec![pane_ids[0], inner_grid])
}
