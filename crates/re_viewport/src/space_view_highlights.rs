use std::collections::BTreeMap;

use egui::NumExt;
use nohash_hasher::IntMap;

use re_log_types::EntityPathHash;
use re_renderer::OutlineMaskPreference;
use re_viewer_context::{
    HoverHighlight, Item, SelectionHighlight, SelectionState, SpaceViewEntityHighlight,
    SpaceViewHighlights, SpaceViewId, SpaceViewOutlineMasks,
};

use crate::SpaceViewBlueprint;

pub fn highlights_for_space_view(
    selection_state: &SelectionState,
    space_view_id: SpaceViewId,
    _space_views: &BTreeMap<SpaceViewId, SpaceViewBlueprint>,
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

    for current_selection in selection_state.current().iter() {
        match current_selection {
            Item::ComponentPath(_) | Item::SpaceView(_) | Item::Container(_) => {}

            Item::DataBlueprintGroup(_space_view_id, _query_id, _entity_path) => {
                // TODO(#4377): Fix DataBlueprintGroup
                /*
                if *group_space_view_id == space_view_id {
                    if let Some(space_view) = space_views.get(group_space_view_id) {
                        // Everything in the same group should receive the same selection outline.
                        // (Due to the way outline masks work in re_renderer, we can't leave the hover channel empty)
                        let selection_mask = next_selection_mask();

                        space_view.contents.visit_group_entities_recursively(
                            *group_handle,
                            &mut |entity_path: &EntityPath| {
                                highlighted_entity_paths
                                    .entry(entity_path.hash())
                                    .or_default()
                                    .overall
                                    .selection = SelectionHighlight::SiblingSelection;
                                let outline_mask_ids =
                                    outlines_masks.entry(entity_path.hash()).or_default();
                                outline_mask_ids.overall =
                                    selection_mask.with_fallback_to(outline_mask_ids.overall);
                            },
                        );
                    }
                }
                */
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

    for current_hover in selection_state.hovered().iter() {
        match current_hover {
            Item::ComponentPath(_) | Item::SpaceView(_) | Item::Container(_) => {}

            Item::DataBlueprintGroup(_space_view_id, _query_id, _entity_path) => {
                // TODO(#4377): Fix DataBlueprintGroup
                /*
                // Unlike for selected objects/data we are more picky for data blueprints with our hover highlights
                // since they are truly local to a space view.
                if *group_space_view_id == space_view_id {
                    if let Some(space_view) = space_views.get(group_space_view_id) {
                        // Everything in the same group should receive the same selection outline.
                        let hover_mask = next_hover_mask();

                        space_view.contents.visit_group_entities_recursively(
                            *group_handle,
                            &mut |entity_path: &EntityPath| {
                                highlighted_entity_paths
                                    .entry(entity_path.hash())
                                    .or_default()
                                    .overall
                                    .hover = HoverHighlight::Hovered;
                                let mask = outlines_masks.entry(entity_path.hash()).or_default();
                                mask.overall = hover_mask.with_fallback_to(mask.overall);
                            },
                        );
                    }
                }
                */
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
