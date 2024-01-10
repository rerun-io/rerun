use egui::NumExt;
use nohash_hasher::IntMap;

use re_log_types::EntityPathHash;
use re_renderer::OutlineMaskPreference;
use re_viewer_context::{
    HoverHighlight, Item, SelectionHighlight, SpaceViewEntityHighlight, SpaceViewHighlights,
    SpaceViewId, SpaceViewOutlineMasks,
};

/// Computes which things in a space view should received highlighting.
///
/// This method makes decisions which entities & instances should which kind of highlighting
/// based on the entities in a space view and the current selection/hover state.
pub fn highlights_for_space_view(
    ctx: &re_viewer_context::ViewerContext<'_>,
    space_view_id: SpaceViewId,
) -> SpaceViewHighlights {
    re_tracing::profile_function!();

    let mut highlighted_entity_paths =
        IntMap::<EntityPathHash, SpaceViewEntityHighlight>::default();
    let mut outlines_masks = IntMap::<EntityPathHash, SpaceViewOutlineMasks>::default();

    let mut selection_mask_index: u8 = 0;
    let mut next_selection_mask = || {
        // We don't expect to overflow u8, but if we do, don't use the "background mask".
        selection_mask_index = selection_mask_index.wrapping_add(1).at_least(1);
        OutlineMaskPreference::some(0, selection_mask_index)
    };

    for current_selection in ctx.selection_state().current().iter_items() {
        match current_selection {
            Item::StoreId(_) | Item::SpaceView(_) | Item::Container(_) => {}

            Item::DataBlueprintGroup(group_space_view_id, query_id, group_entity_path) => {
                // Unlike for selected objects/data we are more picky for data blueprints with our hover highlights
                // since they are truly local to a space view.
                if *group_space_view_id == space_view_id {
                    // Everything in the same group should receive the same selection outline.
                    // (Due to the way outline masks work in re_renderer, we can't leave the hover channel empty)
                    let selection_mask = next_selection_mask();

                    let query_result = ctx.lookup_query_result(*query_id).clone();

                    query_result
                        .tree
                        .visit_group(group_entity_path, &mut |handle| {
                            if let Some(result) = query_result.tree.lookup_result(handle) {
                                let entity_hash = result.entity_path.hash();
                                let instance = result.entity_path.clone().into();

                                highlighted_entity_paths
                                    .entry(entity_hash)
                                    .or_default()
                                    .add_selection(&instance, SelectionHighlight::SiblingSelection);
                                outlines_masks
                                    .entry(entity_hash)
                                    .or_default()
                                    .add(&instance, selection_mask);
                            }
                        });
                }
            }

            Item::ComponentPath(component_path) => {
                let entity_hash = component_path.entity_path.hash();
                let instance = component_path.entity_path.clone().into();

                highlighted_entity_paths
                    .entry(entity_hash)
                    .or_default()
                    .add_selection(&instance, SelectionHighlight::SiblingSelection);
                outlines_masks
                    .entry(entity_hash)
                    .or_default()
                    .add(&instance, next_selection_mask());
            }

            Item::InstancePath(selected_space_view_context, selected_instance) => {
                let entity_hash = selected_instance.entity_path.hash();

                let highlight = if *selected_space_view_context == Some(space_view_id) {
                    SelectionHighlight::Selection
                } else {
                    SelectionHighlight::SiblingSelection
                };
                highlighted_entity_paths
                    .entry(entity_hash)
                    .or_default()
                    .add_selection(selected_instance, highlight);
                outlines_masks
                    .entry(entity_hash)
                    .or_default()
                    .add(selected_instance, next_selection_mask());
            }
        };
    }

    let mut hover_mask_index: u8 = 0;
    let mut next_hover_mask = || {
        // We don't expect to overflow u8, but if we do, don't use the "background mask".
        hover_mask_index = hover_mask_index.wrapping_add(1).at_least(1);
        OutlineMaskPreference::some(hover_mask_index, 0)
    };

    for current_hover in ctx.selection_state().hovered().iter_items() {
        match current_hover {
            Item::StoreId(_) | Item::SpaceView(_) | Item::Container(_) => {}

            Item::DataBlueprintGroup(group_space_view_id, query_id, group_entity_path) => {
                // Unlike for selected objects/data we are more picky for data blueprints with our hover highlights
                // since they are truly local to a space view.
                if *group_space_view_id == space_view_id {
                    // Everything in the same group should receive the same hover outline.
                    let hover_mask = next_hover_mask();

                    let query_result = ctx.lookup_query_result(*query_id).clone();

                    query_result
                        .tree
                        .visit_group(group_entity_path, &mut |handle| {
                            if let Some(result) = query_result.tree.lookup_result(handle) {
                                let instance = result.entity_path.clone().into();
                                highlighted_entity_paths
                                    .entry(result.entity_path.hash())
                                    .or_default()
                                    .add_hover(&instance, HoverHighlight::Hovered);
                                outlines_masks
                                    .entry(result.entity_path.hash())
                                    .or_default()
                                    .add(&instance, hover_mask);
                            }
                        });
                }
            }

            Item::ComponentPath(component_path) => {
                let entity_hash = component_path.entity_path.hash();
                let instance = component_path.entity_path.clone().into();

                highlighted_entity_paths
                    .entry(entity_hash)
                    .or_default()
                    .add_hover(&instance, HoverHighlight::Hovered);
                outlines_masks
                    .entry(entity_hash)
                    .or_default()
                    .add(&instance, next_hover_mask());
            }

            Item::InstancePath(_, selected_instance) => {
                let entity_hash = selected_instance.entity_path.hash();
                highlighted_entity_paths
                    .entry(entity_hash)
                    .or_default()
                    .add_hover(selected_instance, HoverHighlight::Hovered);
                outlines_masks
                    .entry(entity_hash)
                    .or_default()
                    .add(selected_instance, next_hover_mask());
            }
        };
    }

    SpaceViewHighlights {
        highlighted_entity_paths,
        outlines_masks,
    }
}
