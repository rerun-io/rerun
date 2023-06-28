//! Code for automatic layout of space views.
//!
//! This uses some very rough heuristics and have a lot of room for improvement.

use std::collections::BTreeMap;

use itertools::Itertools as _;

use re_viewer_context::SpaceViewId;

use super::{space_view::SpaceViewBlueprint, view_category::ViewCategory};

#[derive(Clone, Debug)]
struct SpaceMakeInfo {
    id: SpaceViewId,
    category: ViewCategory,
}

pub(crate) fn tree_from_space_views(
    space_views: &BTreeMap<SpaceViewId, SpaceViewBlueprint>,
) -> egui_tiles::Tree<SpaceViewId> {
    if space_views.is_empty() {
        return egui_tiles::Tree::empty();
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
        .map(|(space_view_id, space_view)| SpaceMakeInfo {
            id: *space_view_id,
            category: space_view.category,
        })
        .collect_vec();

    let mut tiles = egui_tiles::Tiles::default();

    let root = if space_make_infos.len() == 1 {
        tiles.insert_pane(space_make_infos[0].id)
    } else if space_make_infos.len() == 3 {
        // Special-case this common case (that doesn't fit nicely in a grid!):
        // tile_by_category(&mut tiles, &space_make_infos)
        arrange_three(
            [
                space_make_infos[0].clone(),
                space_make_infos[1].clone(),
                space_make_infos[2].clone(),
            ],
            &mut tiles,
        )
    } else {
        // Arrange it all in a grid that is responsive to changes in viewport size:
        let child_tile_ids = space_make_infos
            .into_iter()
            .map(|smi| tiles.insert_pane(smi.id))
            .collect_vec();
        tiles.insert_grid_tile(child_tile_ids)
    };

    egui_tiles::Tree::new(root, tiles)
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
    // Answer: we prioritize them by category:

    /// lower is better
    fn category_priority(category: ViewCategory) -> usize {
        match category {
            ViewCategory::Spatial => 0,
            ViewCategory::Tensor => 1,
            ViewCategory::TimeSeries => 2,
            ViewCategory::BarChart => 3,
            ViewCategory::TextBox => 4,
            ViewCategory::Text => 5,
        }
    }

    spaces.sort_by_key(|smi| category_priority(smi.category));

    let pane_ids = spaces
        .into_iter()
        .map(|smi| tiles.insert_pane(smi.id))
        .collect_vec();

    let inner_grid = tiles.insert_grid_tile(vec![pane_ids[1], pane_ids[2]]);
    tiles.insert_grid_tile(vec![pane_ids[0], inner_grid])
}
