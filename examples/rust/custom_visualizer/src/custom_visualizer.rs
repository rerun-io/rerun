use rerun::{
    external::{
        re_query, re_renderer, re_types,
        re_view::{DataResultQuery as _, RangeResultsExt as _},
        re_view_spatial,
        re_viewer_context::{
            self, auto_color_for_entity_path, IdentifiedViewSystem, QueryContext,
            TypedComponentFallbackProvider, ViewContext, ViewContextCollection, ViewQuery,
            ViewSystemExecutionError, ViewSystemIdentifier, VisualizerQueryInfo, VisualizerSystem,
        },
    },
    Component as _,
};

use crate::{custom_archetype::Custom, custom_renderer::CustomDrawData};

#[derive(Default)]
pub struct CustomVisualizer {}

impl IdentifiedViewSystem for CustomVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "Custom".into()
    }
}

// TODO: copy pasted out of re_view_spatial, but it's generally useful.
/// Iterate over all the values in the slice, then repeat the last value forever.
///
/// If the input slice is empty, the second argument is returned forever.
#[inline]
pub fn clamped_or<'a, T>(values: &'a [T], if_empty: &'a T) -> impl Iterator<Item = &'a T> + Clone {
    let repeated = values.last().unwrap_or(if_empty);
    values.iter().chain(std::iter::repeat(repeated))
}

impl VisualizerSystem for CustomVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Custom>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let transforms = context_systems.get::<re_view_spatial::TransformTreeContext>()?;
        let render_ctx = ctx.render_ctx();

        let mut draw_data = CustomDrawData::new(render_ctx);

        for data_result in query.iter_visible_data_results(Self::identifier()) {
            let ent_path = &data_result.entity_path;
            let Some(transform_info) = transforms.transform_info_for_entity(ent_path.hash()) else {
                continue; // No valid transform info for this entity.
            };

            let results = data_result.query_archetype_with_history::<Custom>(ctx, query);

            // TODO: handle component instances etc.
            // TODO: handle ziping of primary component and transform info
            // for (instance, transform) in transform_info.reference_from_instances.iter().enumerate()
            let transform = transform_info.reference_from_instances.first();

            // gather all relevant chunks
            let timeline = query.timeline;
            let all_positions = results.iter_as(timeline, rerun::Position3D::name());
            let all_colors = results.iter_as(timeline, rerun::Color::name());

            let picking_layer_object_id = re_renderer::PickingLayerObjectId(ent_path.hash64());
            let entity_outline_mask = query.highlights.entity_outline_mask(ent_path.hash());

            let fallback_color: rerun::Color =
                self.fallback_for(&ctx.query_context(data_result, &query.latest_at_query()));

            for (_index, positions, colors) in re_query::range_zip_1x1(
                all_positions.slice::<[f32; 3]>(),
                all_colors.slice::<u32>(),
            ) {
                let colors: &[rerun::Color] =
                    colors.map_or(&[], |colors| bytemuck::cast_slice(colors));
                let colors = clamped_or(colors, &fallback_color);

                for (instance_index, (position, color)) in
                    positions.into_iter().zip(colors.into_iter()).enumerate()
                {
                    let instance = instance_index as u64;
                    let picking_layer_instance_id = re_renderer::PickingLayerInstanceId(instance);
                    let outline_mask = entity_outline_mask.index_outline_mask(instance.into());

                    draw_data.add(
                        render_ctx,
                        &ent_path.to_string(),
                        *transform,
                        (*color).into(),
                        picking_layer_object_id,
                        picking_layer_instance_id,
                        outline_mask,
                    );
                }
            }
        }

        Ok(vec![draw_data.into()])
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<rerun::Color> for CustomVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> rerun::Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

re_viewer_context::impl_component_fallback_provider!(CustomVisualizer => [rerun::Color]);
