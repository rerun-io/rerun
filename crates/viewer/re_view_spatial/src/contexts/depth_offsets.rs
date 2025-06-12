use std::collections::{BTreeMap, BTreeSet};

use ahash::HashMap;

use re_log_types::EntityPathHash;
use re_types::{ComponentDescriptorSet, Loggable as _, archetypes, components::DrawOrder};
use re_view::latest_at_with_blueprint_resolved_data;
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewContextSystem, ViewSystemIdentifier,
};

use crate::visualizers::visualizers_processing_draw_order;

/// Context for creating a mapping from [`DrawOrder`] to [`re_renderer::DepthOffset`].
#[derive(Default)]
pub struct EntityDepthOffsets {
    pub per_entity_and_visualizer:
        HashMap<(ViewSystemIdentifier, EntityPathHash), re_renderer::DepthOffset>,
}

impl IdentifiedViewSystem for EntityDepthOffsets {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "EntityDepthOffsets".into()
    }
}

impl ViewContextSystem for EntityDepthOffsets {
    fn compatible_component_sets(&self) -> Vec<ComponentDescriptorSet> {
        vec![
            [
                archetypes::Arrows2D::descriptor_indicator(),
                archetypes::Boxes2D::descriptor_indicator(),
                archetypes::DepthImage::descriptor_indicator(),
                archetypes::EncodedImage::descriptor_indicator(),
                archetypes::Image::descriptor_indicator(),
                archetypes::LineStrips2D::descriptor_indicator(),
                archetypes::Points2D::descriptor_indicator(),
                archetypes::SegmentationImage::descriptor_indicator(),
                archetypes::VideoFrameReference::descriptor_indicator(),
                archetypes::VideoStream::descriptor_indicator(),
            ]
            .into_iter()
            .collect(),
        ]
    }

    fn execute(
        &mut self,
        ctx: &re_viewer_context::ViewContext<'_>,
        query: &re_viewer_context::ViewQuery<'_>,
    ) {
        let mut entities_per_draw_order = BTreeMap::new();
        for (visualizer, draw_order_descriptor) in visualizers_processing_draw_order() {
            collect_draw_order_per_visualizer(
                ctx,
                query,
                visualizer,
                &draw_order_descriptor,
                &mut entities_per_draw_order,
            );
        }

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
        let mut depth_offset = -((num_entities_with_draw_order / 2) as re_renderer::DepthOffset);
        self.per_entity_and_visualizer = entities_per_draw_order
            .into_values()
            .flat_map(|keys| {
                keys.into_iter()
                    .map(|key| {
                        depth_offset += 1;
                        (key, depth_offset)
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn collect_draw_order_per_visualizer(
    ctx: &re_viewer_context::ViewContext<'_>,
    query: &re_viewer_context::ViewQuery<'_>,
    visualizer_identifier: ViewSystemIdentifier,
    draw_order_descriptor: &re_types::ComponentDescriptor,
    entities_per_draw_order: &mut BTreeMap<
        DrawOrder,
        BTreeSet<(ViewSystemIdentifier, EntityPathHash)>,
    >,
) {
    let latest_at_query = ctx.current_query();
    let mut default_draw_order = None; // determined lazily

    for data_result in query.iter_visible_data_results(visualizer_identifier) {
        let query_shadowed_components = false;
        let draw_order = latest_at_with_blueprint_resolved_data(
            ctx,
            None,
            &latest_at_query,
            data_result,
            [draw_order_descriptor],
            query_shadowed_components,
        )
        .get_mono::<DrawOrder>(draw_order_descriptor)
        .unwrap_or_else(|| {
            *default_draw_order.get_or_insert_with(|| {
                let ctx = ctx.query_context(data_result, &latest_at_query);
                determine_default_draworder(&ctx, visualizer_identifier, draw_order_descriptor)
            })
        });

        entities_per_draw_order
            .entry(draw_order)
            .or_default()
            .insert((visualizer_identifier, data_result.entity_path.hash()));
    }
}

fn determine_default_draworder(
    ctx: &QueryContext<'_>,
    visualizer_identifier: ViewSystemIdentifier,
    draw_order_descriptor: &re_types::ComponentDescriptor,
) -> DrawOrder {
    let Some(visualizer) = ctx
        .viewer_ctx()
        .view_class_registry()
        .instantiate_visualizer(visualizer_identifier)
    else {
        return DrawOrder::default();
    };

    let draw_order_array = visualizer
        .fallback_provider()
        .fallback_for(ctx, draw_order_descriptor);
    let draw_order_array = DrawOrder::from_arrow(&draw_order_array)
        .ok()
        .unwrap_or_default();
    draw_order_array.first().copied().unwrap_or_default()
}
