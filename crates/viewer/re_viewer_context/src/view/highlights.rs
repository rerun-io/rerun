use nohash_hasher::IntMap;
use re_entity_db::InstancePath;
use re_log_types::{EntityPathHash, Instance};
use re_renderer::OutlineMaskPreference;
use re_sdk_types::blueprint::components::VisualizerInstructionId;

use crate::{HoverHighlight, InteractionHighlight, SelectionHighlight};

/// Highlights of a specific entity path in a specific view.
///
/// Using this in bulk on many instances is faster than querying single objects.
#[derive(Debug)]
pub struct ViewEntityHighlight {
    overall: InteractionHighlight,
    instances: ahash::HashMap<Instance, InteractionHighlight>,

    /// If present, only data from the specified visualizer instruction should be highlighted, otherwise, highlight independently of the visualizer instruction.
    visualizer_instruction: Option<VisualizerInstructionId>,
}

impl ViewEntityHighlight {
    pub fn new(visualizer_instruction: Option<VisualizerInstructionId>) -> Self {
        Self {
            overall: InteractionHighlight::default(),
            instances: ahash::HashMap::default(),
            visualizer_instruction,
        }
    }

    /// Adds a new highlight to the entity highlight, combining it with existing highlights.
    #[inline]
    pub fn add(&mut self, instance: &InstancePath, highlight: InteractionHighlight) {
        let highlight_target = if let Some(selected_index) = instance.instance.specific_index() {
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
pub struct OptionalViewEntityHighlight<'a>(Option<&'a ViewEntityHighlight>);

impl OptionalViewEntityHighlight<'_> {
    #[inline]
    pub fn index_highlight(
        &self,
        instance: Instance,
        visualizer: VisualizerInstructionId,
    ) -> InteractionHighlight {
        match self.0 {
            Some(entity_highlight) => {
                if entity_highlight
                    .visualizer_instruction
                    .is_some_and(|v| v != visualizer)
                {
                    // This highlight is for a different visualizer instruction, so ignore it.
                    InteractionHighlight::default()
                } else {
                    entity_highlight
                        .instances
                        .get(&instance)
                        .copied()
                        .unwrap_or_default()
                        .max(entity_highlight.overall)
                }
            }
            None => InteractionHighlight::default(),
        }
    }
}

#[derive(Default, Debug)]
pub struct ViewOutlineMasks {
    pub overall: OutlineMaskPreference,
    pub instances: ahash::HashMap<Instance, OutlineMaskPreference>,
}

impl ViewOutlineMasks {
    pub fn index_outline_mask(&self, instance: Instance) -> OutlineMaskPreference {
        self.instances
            .get(&instance)
            .copied()
            .unwrap_or_default()
            .with_fallback_to(self.overall)
    }

    /// Add a new outline mask to this entity path, combining it with existing masks.
    pub fn add(&mut self, instance: &InstancePath, preference: OutlineMaskPreference) {
        let outline_mask_target = if let Some(selected_index) = instance.instance.specific_index() {
            self.instances.entry(selected_index).or_default()
        } else {
            &mut self.overall
        };
        *outline_mask_target = preference.with_fallback_to(*outline_mask_target);
    }
}

/// Highlights in a specific view.
///
/// Using this in bulk on many objects is faster than querying single objects.
#[derive(Default, Debug)]
pub struct ViewHighlights {
    pub highlighted_entity_paths: IntMap<EntityPathHash, ViewEntityHighlight>,
    pub outlines_masks: IntMap<EntityPathHash, ViewOutlineMasks>,
}

impl ViewHighlights {
    #[inline]
    pub fn entity_highlight(
        &self,
        entity_path_hash: EntityPathHash,
    ) -> OptionalViewEntityHighlight<'_> {
        OptionalViewEntityHighlight(self.highlighted_entity_paths.get(&entity_path_hash))
    }

    #[inline]
    pub fn entity_outline_mask(&self, entity_path_hash: EntityPathHash) -> &ViewOutlineMasks {
        use std::sync::OnceLock;
        static CELL: OnceLock<ViewOutlineMasks> = OnceLock::new();

        self.outlines_masks
            .get(&entity_path_hash)
            .unwrap_or_else(|| CELL.get_or_init(ViewOutlineMasks::default))
    }

    #[inline]
    pub fn any_outlines(&self) -> bool {
        !self.outlines_masks.is_empty()
    }
}
