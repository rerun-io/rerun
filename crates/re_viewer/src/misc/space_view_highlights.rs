use ahash::HashMap;
use egui::NumExt;
use lazy_static::lazy_static;
use nohash_hasher::IntMap;

use re_log_types::{component_types::InstanceKey, EntityPath, EntityPathHash};
use re_renderer::OutlineMaskPreference;
use re_viewer_context::{
    HoverHighlight, InteractionHighlight, Item, SelectionHighlight, SelectionState, SpaceViewId,
};

use crate::ui::SpaceView;

/// Highlights of a specific entity path in a specific space view.
///
/// Using this in bulk on many instances is faster than querying single objects.
#[derive(Default)]
pub struct SpaceViewEntityHighlight {
    overall: InteractionHighlight,
    instances: ahash::HashMap<InstanceKey, InteractionHighlight>,
}

#[derive(Copy, Clone)]
pub struct OptionalSpaceViewEntityHighlight<'a>(Option<&'a SpaceViewEntityHighlight>);

impl<'a> OptionalSpaceViewEntityHighlight<'a> {
    pub fn index_highlight(&self, instance_key: InstanceKey) -> InteractionHighlight {
        match self.0 {
            Some(entity_highlight) => entity_highlight
                .instances
                .get(&instance_key)
                .cloned()
                .unwrap_or_default()
                .max(entity_highlight.overall),
            None => InteractionHighlight::default(),
        }
    }
}

#[derive(Default)]
pub struct SpaceViewOutlineMasks {
    pub overall: OutlineMaskPreference,
    pub instances: ahash::HashMap<InstanceKey, OutlineMaskPreference>,
    pub any_selection_highlight: bool,
}

lazy_static! {
    static ref SPACEVIEW_OUTLINE_MASK_NONE: SpaceViewOutlineMasks =
        SpaceViewOutlineMasks::default();
}

impl SpaceViewOutlineMasks {
    pub fn index_outline_mask(&self, instance_key: InstanceKey) -> OutlineMaskPreference {
        self.instances
            .get(&instance_key)
            .cloned()
            .unwrap_or_default()
            .with_fallback_to(self.overall)
    }
}

/// Highlights in a specific space view.
///
/// Using this in bulk on many objects is faster than querying single objects.
#[derive(Default)]
pub struct SpaceViewHighlights {
    highlighted_entity_paths: IntMap<EntityPathHash, SpaceViewEntityHighlight>,
    outlines_masks: IntMap<EntityPathHash, SpaceViewOutlineMasks>,
}

impl SpaceViewHighlights {
    pub fn entity_highlight(
        &self,
        entity_path_hash: EntityPathHash,
    ) -> OptionalSpaceViewEntityHighlight<'_> {
        OptionalSpaceViewEntityHighlight(self.highlighted_entity_paths.get(&entity_path_hash))
    }

    pub fn entity_outline_mask(&self, entity_path_hash: EntityPathHash) -> &SpaceViewOutlineMasks {
        self.outlines_masks
            .get(&entity_path_hash)
            .unwrap_or(&SPACEVIEW_OUTLINE_MASK_NONE)
    }

    pub fn any_outlines(&self) -> bool {
        !self.outlines_masks.is_empty()
    }
}

pub fn highlights_for_space_view(
    selection_state: &SelectionState,
    space_view_id: SpaceViewId,
    space_views: &HashMap<SpaceViewId, SpaceView>,
) -> SpaceViewHighlights {
    crate::profile_function!();

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
            Item::ComponentPath(_) | Item::SpaceView(_) => {}

            Item::DataBlueprintGroup(group_space_view_id, group_handle) => {
                if *group_space_view_id == space_view_id {
                    if let Some(space_view) = space_views.get(group_space_view_id) {
                        // Everything in the same group should receive the same selection outline.
                        // (Due to the way outline masks work in re_renderer, we can't leave the hover channel empty)
                        let selection_mask = next_selection_mask();

                        space_view.data_blueprint.visit_group_entities_recursively(
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
                                outline_mask_ids.any_selection_highlight = true;
                            },
                        );
                    }
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
                    outline_mask_ids.any_selection_highlight = true;
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
            Item::ComponentPath(_) | Item::SpaceView(_) => {}

            Item::DataBlueprintGroup(group_space_view_id, group_handle) => {
                // Unlike for selected objects/data we are more picky for data blueprints with our hover highlights
                // since they are truly local to a space view.
                if *group_space_view_id == space_view_id {
                    if let Some(space_view) = space_views.get(group_space_view_id) {
                        // Everything in the same group should receive the same selection outline.
                        let hover_mask = next_hover_mask();

                        space_view.data_blueprint.visit_group_entities_recursively(
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
