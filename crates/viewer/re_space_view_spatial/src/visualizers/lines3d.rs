use re_log_types::Instance;
use re_renderer::PickingLayerInstanceId;
use re_types::{
    archetypes::LineStrips3D,
    components::{ClassId, Color, KeypointId, LineStrip3D, Radius, Text},
    ArrowString, Loggable as _,
};
use re_viewer_context::{
    auto_color_for_entity_path, ApplicableEntities, IdentifiedViewSystem, QueryContext,
    SpaceViewSystemExecutionError, TypedComponentFallbackProvider, ViewContext,
    ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::SpatialSceneEntityContext, view_kind::SpatialSpaceViewKind,
    visualizers::process_labels_3d,
};

use super::{
    filter_visualizable_3d_entities, process_annotation_and_keypoint_slices, process_color_slice,
    process_radius_slice, SpatialViewVisualizerData, SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};

// ---

pub struct Lines3DVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for Lines3DVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::ThreeD)),
        }
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Lines3DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Lines3DComponentData<'a>>,
    ) {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let num_instances = data.strips.len();
            if num_instances == 0 {
                continue;
            }

            let (annotation_infos, _) = process_annotation_and_keypoint_slices(
                query.latest_at,
                num_instances,
                data.strips.iter().map(|_| glam::Vec3::ZERO),
                data.keypoint_ids,
                data.class_ids,
                &ent_context.annotations,
            );

            // Has not custom fallback for radius, so we use the default.
            // TODO(andreas): It would be nice to have this handle this fallback as part of the query.
            let radii =
                process_radius_slice(entity_path, num_instances, data.radii, Radius::default());
            let colors =
                process_color_slice(ctx, self, num_instances, &annotation_infos, data.colors);

            let mut line_batch = line_builder
                .batch(entity_path.to_string())
                .depth_offset(ent_context.depth_offset)
                .world_from_obj(ent_context.world_from_entity)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

            let mut obj_space_bounding_box = re_math::BoundingBox::NOTHING;

            let mut num_rendered_strips = 0usize;
            for (i, (strip, radius, &color)) in
                itertools::izip!(data.strips.iter(), radii, &colors).enumerate()
            {
                let lines = line_batch
                    .add_strip(strip.iter().copied().map(Into::into))
                    .color(color)
                    .radius(radius)
                    .picking_instance_id(PickingLayerInstanceId(i as _));

                if let Some(outline_mask_ids) = ent_context
                    .highlight
                    .instances
                    .get(&Instance::from(i as u64))
                {
                    lines.outline_mask_ids(*outline_mask_ids);
                }

                for p in *strip {
                    obj_space_bounding_box.extend((*p).into());
                }

                num_rendered_strips += 1;
            }
            debug_assert_eq!(data.strips.len(), num_rendered_strips, "the number of renderer strips after all post-processing is done should be equal to {} (got {num_rendered_strips} instead)", data.strips.len());

            self.data.add_bounding_box(
                entity_path.hash(),
                obj_space_bounding_box,
                ent_context.world_from_entity,
            );

            if data.labels.len() == 1 || num_instances <= super::MAX_NUM_LABELS_PER_ENTITY {
                // If there's many strips but only a single label, place the single label at the middle of the visualization.
                let label_positions = if data.labels.len() == 1 && data.strips.len() > 1 {
                    // TODO(andreas): A smoothed over time (+ discontinuity detection) bounding box would be great.
                    itertools::Either::Left(std::iter::once(obj_space_bounding_box.center()))
                } else {
                    // Take middle point of every strip.
                    itertools::Either::Right(data.strips.iter().map(|strip| {
                        strip
                            .iter()
                            .copied()
                            .map(glam::Vec3::from)
                            .sum::<glam::Vec3>()
                            / (strip.len() as f32)
                    }))
                };

                self.data.ui_labels.extend(process_labels_3d(
                    entity_path,
                    label_positions,
                    &data.labels,
                    &colors,
                    &annotation_infos,
                    ent_context.world_from_entity,
                ));
            }
        }
    }
}

// ---

struct Lines3DComponentData<'a> {
    // Point of views
    strips: Vec<&'a [[f32; 3]]>,

    // Clamped to edge
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: Vec<ArrowString>,
    keypoint_ids: &'a [KeypointId],
    class_ids: &'a [ClassId],
}

impl IdentifiedViewSystem for Lines3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Lines3D".into()
    }
}

impl VisualizerSystem for Lines3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<LineStrips3D>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_3d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return Err(SpaceViewSystemExecutionError::NoRenderContextError);
        };

        let mut line_builder = re_renderer::LineDrawableBuilder::new(render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

        use super::entity_iterator::{iter_primitive_array_list, process_archetype};
        process_archetype::<Self, LineStrips3D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_space_view::RangeResultsExt2 as _;

                let Some(all_strip_chunks) = results.get_required_chunks(&LineStrip3D::name())
                else {
                    return Ok(());
                };

                let num_strips = all_strip_chunks
                    .iter()
                    .flat_map(|chunk| {
                        chunk.iter_primitive_array_list::<3, f32>(&LineStrip3D::name())
                    })
                    .map(|strips| strips.len())
                    .sum();
                if num_strips == 0 {
                    return Ok(());
                }
                line_builder.reserve_strips(num_strips)?;

                let num_vertices = all_strip_chunks
                    .iter()
                    .flat_map(|chunk| {
                        chunk.iter_primitive_array_list::<3, f32>(&LineStrip3D::name())
                    })
                    .map(|strips| strips.iter().map(|strip| strip.len()).sum::<usize>())
                    .sum::<usize>();
                line_builder.reserve_vertices(num_vertices)?;

                let timeline = ctx.query.timeline();
                let all_strips_indexed =
                    iter_primitive_array_list(&all_strip_chunks, timeline, LineStrip3D::name());
                let all_colors = results.iter_as(timeline, Color::name());
                let all_radii = results.iter_as(timeline, Radius::name());
                let all_labels = results.iter_as(timeline, Text::name());
                let all_class_ids = results.iter_as(timeline, ClassId::name());
                let all_keypoint_ids = results.iter_as(timeline, KeypointId::name());

                let data = re_query2::range_zip_1x5(
                    all_strips_indexed,
                    all_colors.primitive::<u32>(),
                    all_radii.primitive::<f32>(),
                    all_labels.string(),
                    all_class_ids.primitive::<u16>(),
                    all_keypoint_ids.primitive::<u16>(),
                )
                .map(
                    |(_index, strips, colors, radii, labels, class_ids, keypoint_ids)| {
                        Lines3DComponentData {
                            strips,
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            radii: radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids
                                .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                            keypoint_ids: keypoint_ids
                                .map_or(&[], |keypoint_ids| bytemuck::cast_slice(keypoint_ids)),
                        }
                    },
                );

                self.process_data(ctx, &mut line_builder, view_query, spatial_ctx, data);

                Ok(())
            },
        )?;

        Ok(vec![(line_builder.into_draw_data()?.into())])
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<Color> for Lines3DVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

re_viewer_context::impl_component_fallback_provider!(Lines3DVisualizer => [Color]);
