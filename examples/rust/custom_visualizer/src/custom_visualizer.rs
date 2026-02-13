use rerun::{
    external::{
        re_query, re_renderer,
        re_view::{DataResultQuery as _, VisualizerInstructionQueryResults},
        re_view_spatial,
        re_viewer_context::{
            self, auto_color_for_entity_path, IdentifiedViewSystem, ViewContext,
            ViewContextCollection, ViewQuery, ViewSystemExecutionError, ViewSystemIdentifier,
            VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
        },
    },
    Archetype as _,
};

use crate::{custom_archetype::Custom, custom_renderer::CustomDrawData};

#[derive(Default)]
pub struct CustomVisualizer {}

impl IdentifiedViewSystem for CustomVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "Custom".into()
    }
}

/// Iterate over all the values in the slice, then repeat the last value forever.
///
/// If the input slice is empty, the second argument is returned forever.
#[inline]
pub fn clamped_or<'a, T>(values: &'a [T], if_empty: &'a T) -> impl Iterator<Item = &'a T> + Clone {
    let repeated = values.last().unwrap_or(if_empty);
    values.iter().chain(std::iter::repeat(repeated))
}

impl VisualizerSystem for CustomVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Custom>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let render_ctx = ctx.render_ctx();

        let mut output = VisualizerExecutionOutput::default();
        let transforms = context_systems.get::<re_view_spatial::TransformTreeContext>(&output)?;
        let mut draw_data = CustomDrawData::new(render_ctx);

        for (data_result, instruction) in
            query.iter_visualizer_instruction_for(Self::identifier())
        {
            let ent_path = &data_result.entity_path;
            let Some(Ok(transform_info)) =
                transforms.target_from_entity_path(ent_path.hash())
            else {
                continue; // No valid transform info for this entity.
            };

            let results =
                data_result.query_archetype_with_history::<Custom>(ctx, query, instruction);
            let results =
                VisualizerInstructionQueryResults::new(instruction.id, &results, &output);

            // Use single_transform_required_for_entity since we only support one transform per entity.
            let transform = transform_info
                .single_transform_required_for_entity(ent_path, Custom::name());

            // Gather all relevant chunks.
            let all_positions =
                results.iter_required(Custom::descriptor_positions().component);
            let all_colors =
                results.iter_optional(Custom::descriptor_colors().component);

            let picking_layer_object_id = re_renderer::PickingLayerObjectId(ent_path.hash64());
            let entity_outline_mask = query.highlights.entity_outline_mask(ent_path.hash());

            let fallback_color: rerun::Color = auto_color_for_entity_path(ent_path);

            for (_index, positions, colors) in re_query::range_zip_1x1(
                all_positions.slice::<[f32; 3]>(),
                all_colors.slice::<u32>(),
            ) {
                let colors: &[rerun::Color] =
                    colors.map_or(&[], |colors| bytemuck::cast_slice(colors));
                let colors = clamped_or(colors, &fallback_color);

                for (instance_index, (_position, color)) in
                    positions.iter().zip(colors.into_iter()).enumerate()
                {
                    let instance = instance_index as u64;
                    let picking_layer_instance_id = re_renderer::PickingLayerInstanceId(instance);
                    let outline_mask = entity_outline_mask.index_outline_mask(instance.into());

                    draw_data.add(
                        render_ctx,
                        &ent_path.to_string(),
                        transform.as_affine3a(),
                        (*color).into(),
                        picking_layer_object_id,
                        picking_layer_instance_id,
                        outline_mask,
                    );
                }
            }
        }

        output.draw_data = vec![draw_data.into()];
        Ok(output)
    }
}

