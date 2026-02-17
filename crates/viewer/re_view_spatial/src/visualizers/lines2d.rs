use re_log_types::Instance;
use re_renderer::renderer::LineStripFlags;
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId};
use re_sdk_types::archetypes::LineStrips2D;
use re_sdk_types::components::{ClassId, Color, Radius, ShowLabels};
use re_sdk_types::{Archetype as _, ArrowString};
use re_view::{process_annotation_slices, process_color_slice};
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
    typed_fallback_for,
};

use super::utilities::{LabeledBatch, process_labels_2d};
use super::{SpatialViewVisualizerData, process_radius_slice};
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::view_kind::SpatialViewKind;

// ---

pub struct Lines2DVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for Lines2DVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::TwoD)),
        }
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Lines2DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        data: impl Iterator<Item = Lines2DComponentData<'a>>,
    ) {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let num_instances = data.strips.len();
            if num_instances == 0 {
                continue;
            }

            let annotation_infos = process_annotation_slices(
                query.latest_at,
                num_instances,
                data.class_ids,
                &ent_context.annotations,
            );

            // Has not custom fallback for radius, so we use the default.
            // TODO(andreas): It would be nice to have this handle this fallback as part of the query.
            let radii =
                process_radius_slice(entity_path, num_instances, data.radii, Radius::default());
            let colors = process_color_slice(
                ctx,
                LineStrips2D::descriptor_colors().component,
                num_instances,
                &annotation_infos,
                data.colors,
            );

            let world_from_obj = ent_context
                .transform_info
                .single_transform_required_for_entity(entity_path, LineStrips2D::name())
                .as_affine3a();

            let mut line_batch = line_builder
                .batch(entity_path.to_string())
                .depth_offset(ent_context.depth_offset)
                .world_from_obj(world_from_obj)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

            let mut obj_space_bounding_box = macaw::BoundingBox::nothing();
            for (i, (strip, radius, &color)) in
                itertools::izip!(data.strips.iter(), radii, &colors).enumerate()
            {
                let lines = line_batch
                    .add_strip_2d(strip.iter().copied().map(Into::into))
                    .color(color)
                    .radius(radius)
                    // Looped lines should be connected with rounded corners, so we always add outward extending caps.
                    .flags(LineStripFlags::FLAGS_OUTWARD_EXTENDING_ROUND_CAPS)
                    .picking_instance_id(PickingLayerInstanceId(i as _));

                if let Some(outline_mask_ids) = ent_context
                    .highlight
                    .instances
                    .get(&Instance::from(i as u64))
                {
                    lines.outline_mask_ids(*outline_mask_ids);
                }

                for p in *strip {
                    obj_space_bounding_box.extend(glam::vec3(p[0], p[1], 0.0));
                }
            }

            self.data
                .add_bounding_box(entity_path.hash(), obj_space_bounding_box, world_from_obj);

            self.data.ui_labels.extend(process_labels_2d(
                LabeledBatch {
                    entity_path,
                    visualizer_instruction: ent_context.visualizer_instruction,
                    num_instances,
                    overall_position: obj_space_bounding_box.center().truncate(),
                    instance_positions: data.strips.iter().map(|strip| {
                        strip
                            .iter()
                            .copied()
                            .map(glam::Vec2::from)
                            .sum::<glam::Vec2>()
                            / (strip.len() as f32)
                    }),
                    labels: &data.labels,
                    colors: &colors,
                    show_labels: data.show_labels.unwrap_or_else(|| {
                        typed_fallback_for(ctx, LineStrips2D::descriptor_show_labels().component)
                    }),
                    annotation_infos: &annotation_infos,
                },
                world_from_obj,
            ));
        }
    }
}

// ---

struct Lines2DComponentData<'a> {
    // Point of views
    strips: Vec<&'a [[f32; 2]]>,

    // Clamped to edge
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: Vec<ArrowString>,
    class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
}

impl IdentifiedViewSystem for Lines2DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Lines2D".into()
    }
}

impl VisualizerSystem for Lines2DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<LineStrips2D>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let output = VisualizerExecutionOutput::default();

        let mut line_builder = re_renderer::LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        use super::entity_iterator::process_archetype;
        process_archetype::<Self, LineStrips2D, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self.data.preferred_view_kind,
            |ctx, spatial_ctx, results| {
                let all_strips = results.iter_required(LineStrips2D::descriptor_strips().component);
                if all_strips.is_empty() {
                    return Ok(());
                }

                // TODO(andreas): Introduce a utility for this?
                let num_strips = all_strips
                    .chunks()
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<&[[f32; 2]]>())
                    .map(|strips| strips.len())
                    .sum();
                if num_strips == 0 {
                    return Ok(());
                }
                line_builder.reserve_strips(num_strips)?;

                let num_vertices = all_strips
                    .chunks()
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<&[[f32; 2]]>())
                    .map(|strips| strips.iter().map(|strip| strip.len()).sum::<usize>())
                    .sum::<usize>();
                line_builder.reserve_vertices(num_vertices)?;
                let all_colors = results.iter_optional(LineStrips2D::descriptor_colors().component);
                let all_radii = results.iter_optional(LineStrips2D::descriptor_radii().component);
                let all_labels = results.iter_optional(LineStrips2D::descriptor_labels().component);
                let all_class_ids =
                    results.iter_optional(LineStrips2D::descriptor_class_ids().component);
                let all_show_labels =
                    results.iter_optional(LineStrips2D::descriptor_show_labels().component);

                let data = re_query::range_zip_1x5(
                    all_strips.slice::<&[[f32; 2]]>(),
                    all_colors.slice::<u32>(),
                    all_radii.slice::<f32>(),
                    all_labels.slice::<String>(),
                    all_class_ids.slice::<u16>(),
                    all_show_labels.slice::<bool>(),
                )
                .map(
                    |(_index, strips, colors, radii, labels, class_ids, show_labels)| {
                        Lines2DComponentData {
                            strips,
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            radii: radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids
                                .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                            show_labels: show_labels
                                .map(|b| !b.is_empty() && b.value(0))
                                .map(Into::into),
                        }
                    },
                );

                self.process_data(ctx, &mut line_builder, view_query, spatial_ctx, data);

                Ok(())
            },
        )?;

        Ok(output.with_draw_data([(line_builder.into_draw_data()?.into())]))
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }
}
