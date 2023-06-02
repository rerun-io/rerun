use lazy_static::lazy_static;
use nohash_hasher::IntMap;

use re_log_types::{EntityPathHash, InstanceKey};
use re_renderer::OutlineMaskPreference;

use crate::InteractionHighlight;

/// Highlights of a specific entity path in a specific space view.
///
/// Using this in bulk on many instances is faster than querying single objects.
#[derive(Default)]
pub struct SpaceViewEntityHighlight {
    pub overall: InteractionHighlight,
    pub instances: ahash::HashMap<InstanceKey, InteractionHighlight>,
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
    pub highlighted_entity_paths: IntMap<EntityPathHash, SpaceViewEntityHighlight>,
    pub outlines_masks: IntMap<EntityPathHash, SpaceViewOutlineMasks>,
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
