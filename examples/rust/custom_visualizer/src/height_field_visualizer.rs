use rerun::Archetype as _;
use rerun::components::{Colormap, ImageFormat};
use rerun::external::re_view::{DataResultQuery as _, VisualizerInstructionQueryResults};
use rerun::external::re_viewer_context::{
    self, IdentifiedViewSystem, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, ViewSystemIdentifier, VisualizerExecutionOutput, VisualizerQueryInfo,
    VisualizerSystem,
};
use rerun::external::{re_query, re_renderer, re_view_spatial};

use crate::height_field_archetype::HeightField;
use crate::height_field_renderer::{HeightFieldConfig, HeightFieldDrawData};

pub const DEFAULT_COLOR_MAP: Colormap = Colormap::Spectral;

/// Visualizer that queries [`HeightField`] data and produces [`HeightFieldDrawData`] for rendering.
#[derive(Default)]
pub struct HeightFieldVisualizer {}

impl IdentifiedViewSystem for HeightFieldVisualizer {
    fn identifier() -> ViewSystemIdentifier {
        "HeightField".into()
    }
}

impl VisualizerSystem for HeightFieldVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<HeightField>()
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
        let mut draw_data = HeightFieldDrawData::new(render_ctx);

        for (data_result, instruction) in query.iter_visualizer_instruction_for(Self::identifier())
        {
            let ent_path = &data_result.entity_path;
            let Some(Ok(transform_info)) = transforms.target_from_entity_path(ent_path.hash())
            else {
                continue;
            };

            let results =
                data_result.query_archetype_with_history::<HeightField>(ctx, query, instruction);
            let results = VisualizerInstructionQueryResults::new(instruction.id, &results, &output);

            let transform =
                transform_info.single_transform_required_for_entity(ent_path, HeightField::name());

            let all_buffers = results.iter_required(HeightField::descriptor_buffer().component);
            let all_formats = results.iter_optional(HeightField::descriptor_format().component);
            let all_colormaps = results.iter_optional(HeightField::descriptor_colormap().component);

            let picking_layer_object_id = re_renderer::PickingLayerObjectId(ent_path.hash64());
            let entity_outline_mask = query.highlights.entity_outline_mask(ent_path.hash());
            let outline_mask = entity_outline_mask.index_outline_mask(0u64.into());

            for (_index, buffer, format, colormap) in re_query::range_zip_1x2(
                all_buffers.slice::<&[u8]>(),
                all_formats.component_slow::<ImageFormat>(),
                all_colormaps.slice::<u8>(),
            ) {
                let Some(buffer) = buffer.first() else {
                    continue;
                };

                let Some(format) = format.as_deref().and_then(|f| f.first()).copied() else {
                    continue;
                };

                let cols = format.0.width;
                let rows = format.0.height;
                if cols < 2 || rows < 2 {
                    continue;
                }

                // Interpret the raw buffer as f32 heights.
                let heights_f32: &[f32] = bytemuck::cast_slice(buffer);
                let expected_len = (rows * cols) as usize;
                if heights_f32.len() < expected_len {
                    continue;
                }
                let heights_f32 = &heights_f32[..expected_len];

                // Compute min/max on CPU for normalization in the shader.
                // It would be nice to do this on the GPU and/or cache it. But for this example we'll keep it simple.
                let min_height = heights_f32.iter().copied().fold(f32::INFINITY, f32::min);
                let max_height = heights_f32
                    .iter()
                    .copied()
                    .fold(f32::NEG_INFINITY, f32::max);

                // Get colormap ID, defaulting to Turbo.
                let colormap_id = colormap
                    .and_then(|c| c.first().copied())
                    .and_then(Colormap::from_u8)
                    .unwrap_or(DEFAULT_COLOR_MAP) as u32;

                let spacing = 10.0 / (cols.max(rows) - 1) as f32;

                draw_data.add_mesh(
                    render_ctx,
                    &ent_path.to_string(),
                    &HeightFieldConfig {
                        world_from_obj: transform.as_affine3a(),
                        heights: heights_f32,
                        grid_cols: cols,
                        grid_rows: rows,
                        spacing,
                        min_height,
                        max_height,
                        colormap: colormap_id,
                        picking_layer_object_id,
                        picking_instance_id: re_renderer::PickingLayerInstanceId(0),
                        outline_mask,
                    },
                );
            }
        }

        output.draw_data = vec![draw_data.into()];
        Ok(output)
    }
}
