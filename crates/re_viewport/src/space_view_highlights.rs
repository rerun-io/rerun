use egui::NumExt;
use nohash_hasher::IntMap;

use re_log_types::EntityPathHash;
use re_renderer::OutlineMaskPreference;
use re_viewer_context::{
    HoverHighlight, Item, SelectionHighlight, SpaceViewEntityHighlight, SpaceViewHighlights,
    SpaceViewId, SpaceViewOutlineMasks,
};

pub fn highlights_for_space_view(
    ctx: &re_viewer_context::ViewerContext<'_>,
    space_view_id: SpaceViewId,
) -> SpaceViewHighlights {
    re_tracing::profile_function!();

    let mut highlighted_entity_paths =
        IntMap::<EntityPathHash, SpaceViewEntityHighlight>::default();
    let mut outlines_masks = IntMap::<EntityPathHash, SpaceViewOutlineMasks>::default();

    let mut selection_mask_index: u8 = 0;
    let mut hover_mask_index: u8 = 0;
    let mut next_selection_mask = || {
        // We don't expect to overflow u8, but if we do, don't use the "background mask".
        selection_mask_index = selection_mask_index.wrapping_add(1).at_least(1);
        OutlineMaskPreference::some(0, selection_mask_index)
    };
    let mut next_hover_mask = || {
        // We don't expect to overflow u8, but if we do, don't use the "background mask".
        hover_mask_index = hover_mask_index.wrapping_add(1).at_least(1);
        OutlineMaskPreference::some(hover_mask_index, 0)
    };

    for current_selection in ctx.selection_state().current().iter_items() {
        match current_selection {
            Item::StoreId(_) | Item::ComponentPath(_) | Item::SpaceView(_) | Item::Container(_) => {
            }

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
                                highlighted_entity_paths
                                    .entry(result.entity_path.hash())
                                    .or_default()
                                    .overall
                                    .selection = SelectionHighlight::SiblingSelection;
                                let outline_mask_ids =
                                    outlines_masks.entry(result.entity_path.hash()).or_default();
                                outline_mask_ids.overall =
                                    selection_mask.with_fallback_to(outline_mask_ids.overall);
                            }
                        });
                }
            }

            Item::InstancePath(selected_space_view_context, selected_instance) => {
                {
                    let highlight = if *selected_space_view_context == Some(space_view_id) {
                        SelectionHighlight::Selection
                    } else {
                        SelectionHighlight::SiblingSelection
                    };

                    let highlighted_entity = highlighted_entity_paths
                        .entry(selected_instance.entity_path.hash())
                        .or_default();
                    let highlight_target = if let Some(selected_index) =
                        selected_instance.instance_key.specific_index()
                    {
                        &mut highlighted_entity
                            .instances
                            .entry(selected_index)
                            .or_default()
                            .selection
                    } else {
                        &mut highlighted_entity.overall.selection
                    };
                    *highlight_target = (*highlight_target).max(highlight);
                }
                {
                    let outline_mask_ids = outlines_masks
                        .entry(selected_instance.entity_path.hash())
                        .or_default();
                    let outline_mask_target = if let Some(selected_index) =
                        selected_instance.instance_key.specific_index()
                    {
                        outline_mask_ids
                            .instances
                            .entry(selected_index)
                            .or_default()
                    } else {
                        &mut outline_mask_ids.overall
                    };
                    *outline_mask_target =
                        next_selection_mask().with_fallback_to(*outline_mask_target);
                }
            }
        };
    }

    for current_hover in ctx.selection_state().hovered().iter_items() {
        match current_hover {
            Item::StoreId(_) | Item::ComponentPath(_) | Item::SpaceView(_) | Item::Container(_) => {
            }

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
                                highlighted_entity_paths
                                    .entry(result.entity_path.hash())
                                    .or_default()
                                    .overall
                                    .hover = HoverHighlight::Hovered;
                                let mask =
                                    outlines_masks.entry(result.entity_path.hash()).or_default();
                                mask.overall = hover_mask.with_fallback_to(mask.overall);
                            }
                        });
                }
            }

            Item::InstancePath(_, selected_instance) => {
                {
                    let highlighted_entity = highlighted_entity_paths
                        .entry(selected_instance.entity_path.hash())
                        .or_default();

                    let highlight_target = if let Some(selected_index) =
                        selected_instance.instance_key.specific_index()
                    {
                        &mut highlighted_entity
                            .instances
                            .entry(selected_index)
                            .or_default()
                            .hover
                    } else {
                        &mut highlighted_entity.overall.hover
                    };
                    *highlight_target = HoverHighlight::Hovered;
                }
                {
                    let outlined_entity = outlines_masks
                        .entry(selected_instance.entity_path.hash())
                        .or_default();
                    let outline_mask_target = if let Some(selected_index) =
                        selected_instance.instance_key.specific_index()
                    {
                        outlined_entity.instances.entry(selected_index).or_default()
                    } else {
                        &mut outlined_entity.overall
                    };
                    *outline_mask_target = next_hover_mask().with_fallback_to(*outline_mask_target);
                }
            }
        };
    }

    SpaceViewHighlights {
        highlighted_entity_paths,
        outlines_masks,
    }
}
