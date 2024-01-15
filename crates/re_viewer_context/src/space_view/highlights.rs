use nohash_hasher::IntMap;

use re_entity_db::InstancePath;
use re_log_types::EntityPathHash;
use re_renderer::OutlineMaskPreference;
use re_types::components::InstanceKey;

use crate::{HoverHighlight, InteractionHighlight, SelectionHighlight};

/// Highlights of a specific entity path in a specific space view.
///
/// Using this in bulk on many instances is faster than querying single objects.
#[derive(Default)]
pub struct SpaceViewEntityHighlight {
    overall: InteractionHighlight,
    instances: ahash::HashMap<InstanceKey, InteractionHighlight>,
}

impl SpaceViewEntityHighlight {
    /// Adds a new highlight to the entity highlight, combining it with existing highlights.
    #[inline]
    pub fn add(&mut self, instance: &InstancePath, highlight: InteractionHighlight) {
        let highlight_target = if let Some(selected_index) = instance.instance_key.specific_index()
        {
            self.instances.entry(selected_index).or_default()
        } else {
            &mut self.overall
        };
        *highlight_target = (*highlight_target).max(highlight);
    }

    /// Adds a new selection highlight to the entity highlight, combining it with existing highlights.
    #[inline]
    pub fn add_selection(&mut self, instance: &InstancePath, selection: SelectionHighlight) {
        self.add(
            instance,
            InteractionHighlight {
                selection,
                hover: HoverHighlight::None,
            },
        );
    }

    /// Adds a new hover highlight to the entity highlight, combining it with existing highlights.
    #[inline]
    pub fn add_hover(&mut self, instance: &InstancePath, hover: HoverHighlight) {
        self.add(
            instance,
            InteractionHighlight {
                selection: SelectionHighlight::None,
                hover,
            },
        );
    }
}

#[derive(Copy, Clone)]
pub struct OptionalSpaceViewEntityHighlight<'a>(Option<&'a SpaceViewEntityHighlight>);

impl<'a> OptionalSpaceViewEntityHighlight<'a> {
    #[inline]
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
}

impl SpaceViewOutlineMasks {
    pub fn index_outline_mask(&self, instance_key: InstanceKey) -> OutlineMaskPreference {
        self.instances
            .get(&instance_key)
            .cloned()
            .unwrap_or_default()
            .with_fallback_to(self.overall)
    }

    /// Add a new outline mask to this entity path, combining it with existing masks.
    pub fn add(&mut self, instance: &InstancePath, preference: OutlineMaskPreference) {
        let outline_mask_target =
            if let Some(selected_index) = instance.instance_key.specific_index() {
                self.instances.entry(selected_index).or_default()
            } else {
                &mut self.overall
            };
        *outline_mask_target = preference.with_fallback_to(*outline_mask_target);
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
    #[inline]
    pub fn entity_highlight(
        &self,
        entity_path_hash: EntityPathHash,
    ) -> OptionalSpaceViewEntityHighlight<'_> {
        OptionalSpaceViewEntityHighlight(self.highlighted_entity_paths.get(&entity_path_hash))
    }

    #[inline]
    pub fn entity_outline_mask(&self, entity_path_hash: EntityPathHash) -> &SpaceViewOutlineMasks {
        use std::sync::OnceLock;
        static CELL: OnceLock<SpaceViewOutlineMasks> = OnceLock::new();

        self.outlines_masks
            .get(&entity_path_hash)
            .unwrap_or_else(|| CELL.get_or_init(SpaceViewOutlineMasks::default))
    }

    #[inline]
    pub fn any_outlines(&self) -> bool {
        !self.outlines_masks.is_empty()
    }
}
