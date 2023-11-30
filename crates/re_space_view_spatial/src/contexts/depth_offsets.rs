use std::collections::{BTreeMap, BTreeSet};

use nohash_hasher::IntMap;

use re_log_types::EntityPathHash;
use re_types::{components::DrawOrder, ComponentNameSet, Loggable as _};
use re_viewer_context::{IdentifiedViewSystem, ViewContextSystem};

/// Context for creating a mapping from [`DrawOrder`] to [`re_renderer::DepthOffset`].
#[derive(Default)]
pub struct EntityDepthOffsets {
    // TODO(wumpf): Given that archetypes (should) contain DrawData, we should have a map of DrawData to DepthOffset.
    //              Mapping entities to depth offset instead is inconsistent with the archetype queries which are
    //              expected to care about DepthOffset iff they can make use of it.
    pub per_entity: IntMap<EntityPathHash, re_renderer::DepthOffset>,
    pub box2d: re_renderer::DepthOffset,
    pub lines2d: re_renderer::DepthOffset,
    pub image: re_renderer::DepthOffset,
    pub points: re_renderer::DepthOffset,
}

impl IdentifiedViewSystem for EntityDepthOffsets {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "EntityDepthOffsets".into()
    }
}

impl ViewContextSystem for EntityDepthOffsets {
    fn compatible_component_sets(&self) -> Vec<ComponentNameSet> {
        vec![std::iter::once(DrawOrder::name()).collect()]
    }

    fn execute(
        &mut self,
        ctx: &mut re_viewer_context::ViewerContext<'_>,
        query: &re_viewer_context::ViewQuery<'_>,
    ) {
        #[derive(PartialEq, PartialOrd, Eq, Ord)]
        enum DrawOrderTarget {
            Entity(EntityPathHash),
            DefaultBox2D,
            DefaultLines2D,
            DefaultImage,
            DefaultPoints,
        }

        let store = ctx.store_db.store();

        // Use a BTreeSet for entity hashes to get a stable order.
        let mut entities_per_draw_order = BTreeMap::<DrawOrder, BTreeSet<DrawOrderTarget>>::new();
        for data_result in query.iter_visible_data_results(Self::identifier()) {
            if let Some(draw_order) = store.query_latest_component::<DrawOrder>(
                &data_result.entity_path,
                &ctx.rec_cfg.time_ctrl.read().current_query(),
            ) {
                entities_per_draw_order
                    .entry(draw_order.value)
                    .or_default()
                    .insert(DrawOrderTarget::Entity(data_result.entity_path.hash()));
            }
        }

        // Push in default draw orders. All of them using the none hash.
        entities_per_draw_order.insert(
            DrawOrder::DEFAULT_BOX2D,
            [DrawOrderTarget::DefaultBox2D].into(),
        );
        entities_per_draw_order.insert(
            DrawOrder::DEFAULT_IMAGE,
            [DrawOrderTarget::DefaultImage].into(),
        );
        entities_per_draw_order.insert(
            DrawOrder::DEFAULT_LINES2D,
            [DrawOrderTarget::DefaultLines2D].into(),
        );
        entities_per_draw_order.insert(
            DrawOrder::DEFAULT_POINTS2D,
            [DrawOrderTarget::DefaultPoints].into(),
        );

        // Determine re_renderer draw order from this.
        //
        // We give objects with the same `DrawOrder` still a different depth offset
        // in order to avoid z-fighting artifacts when rendering in 3D.
        // (for pure 2D this isn't necessary)
        //
        // We want to be as tightly around 0 as possible.
        let num_entities_with_draw_order: usize = entities_per_draw_order
            .values()
            .map(|entities| entities.len())
            .sum();
        let mut draw_order = -((num_entities_with_draw_order / 2) as re_renderer::DepthOffset);
        self.per_entity = entities_per_draw_order
            .into_values()
            .flat_map(|targets| {
                targets
                    .into_iter()
                    .filter_map(|target| {
                        draw_order += 1;
                        match target {
                            DrawOrderTarget::Entity(entity) => Some((entity, draw_order)),
                            DrawOrderTarget::DefaultBox2D => {
                                self.box2d = draw_order;
                                None
                            }
                            DrawOrderTarget::DefaultLines2D => {
                                self.lines2d = draw_order;
                                None
                            }
                            DrawOrderTarget::DefaultImage => {
                                self.image = draw_order;
                                None
                            }
                            DrawOrderTarget::DefaultPoints => {
                                self.points = draw_order;
                                None
                            }
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
