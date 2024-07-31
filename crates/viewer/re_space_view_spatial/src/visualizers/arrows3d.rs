use re_log_types::Instance;
use re_query::range_zip_1x6;
use re_renderer::{renderer::LineStripFlags, LineDrawableBuilder, PickingLayerInstanceId};
use re_types::{
    archetypes::Arrows3D,
    components::{ClassId, Color, KeypointId, Position3D, Radius, Text, Vector3D},
};
use re_viewer_context::{
    auto_color_for_entity_path, ApplicableEntities, IdentifiedViewSystem, QueryContext,
    SpaceViewSystemExecutionError, TypedComponentFallbackProvider, ViewContext,
    ViewContextCollection, ViewQuery, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::SpatialSceneEntityContext, view_kind::SpatialSpaceViewKind,
    visualizers::filter_visualizable_3d_entities,
};

use super::{
    entity_iterator::clamped_or, process_annotation_and_keypoint_slices, process_color_slice,
    process_labels_3d, process_radius_slice, SpatialViewVisualizerData,
    SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
};

// ---

pub struct Arrows3DVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for Arrows3DVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::ThreeD)),
        }
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Arrows3DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Arrows3DComponentData<'a>>,
    ) {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let num_instances = data.vectors.len();
            if num_instances == 0 {
                continue;
            }

            let (annotation_infos, _) = process_annotation_and_keypoint_slices(
                query.latest_at,
                num_instances,
                data.vectors.iter().map(|_| glam::Vec3::ZERO),
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

            let world_from_obj = ent_context
                .transform_info
                .single_entity_transform_required(entity_path, "Arrows3D");

            let mut line_batch = line_builder
                .batch(entity_path.to_string())
                .world_from_obj(world_from_obj)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

            let mut obj_space_bounding_box = re_math::BoundingBox::NOTHING;

            let origins = clamped_or(data.origins, &Position3D::ZERO);

            for (i, (vector, origin, radius, &color)) in
                itertools::izip!(data.vectors, origins, radii, &colors).enumerate()
            {
                let vector: glam::Vec3 = vector.0.into();
                let origin: glam::Vec3 = origin.0.into();
                let end = origin + vector;

                let segment = line_batch
                    .add_segment(origin, end)
                    .radius(radius)
                    .color(color)
                    .flags(
                        LineStripFlags::FLAG_COLOR_GRADIENT
                            | LineStripFlags::FLAG_CAP_END_TRIANGLE
                            | LineStripFlags::FLAG_CAP_START_ROUND
                            | LineStripFlags::FLAG_CAP_START_EXTEND_OUTWARDS,
                    )
                    .picking_instance_id(PickingLayerInstanceId(i as _));

                if let Some(outline_mask_ids) = ent_context
                    .highlight
                    .instances
                    .get(&Instance::from(i as u64))
                {
                    segment.outline_mask_ids(*outline_mask_ids);
                }

                obj_space_bounding_box.extend(origin);
                obj_space_bounding_box.extend(end);
            }

            self.data
                .add_bounding_box(entity_path.hash(), obj_space_bounding_box, world_from_obj);

            if data.labels.len() == 1 || num_instances <= super::MAX_NUM_LABELS_PER_ENTITY {
                // If there's many arrows but only a single label, place the single label at the middle of the visualization.
                let label_positions = if data.labels.len() == 1 && num_instances > 1 {
                    // TODO(andreas): A smoothed over time (+ discontinuity detection) bounding box would be great.
                    itertools::Either::Left(std::iter::once(obj_space_bounding_box.center()))
                } else {
                    // Take middle point of every arrow.
                    let origins = clamped_or(data.origins, &Position3D::ZERO);

                    itertools::Either::Right(itertools::izip!(data.vectors, origins).map(
                        |(vector, origin)| {
                            // `0.45` rather than `0.5` to account for cap and such
                            glam::Vec3::from(origin.0) + glam::Vec3::from(vector.0) * 0.45
                        },
                    ))
                };

                self.data.ui_labels.extend(process_labels_3d(
                    entity_path,
                    label_positions,
                    data.labels,
                    &colors,
                    &annotation_infos,
                    world_from_obj,
                ));
            }
        }
    }
}

// ---

struct Arrows3DComponentData<'a> {
    // Point of views
    vectors: &'a [Vector3D],

    // Clamped to edge
    origins: &'a [Position3D],
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: &'a [Text],
    keypoint_ids: &'a [KeypointId],
    class_ids: &'a [ClassId],
}

impl IdentifiedViewSystem for Arrows3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Arrows3D".into()
    }
}

impl VisualizerSystem for Arrows3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Arrows3D>()
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

        let mut line_builder = LineDrawableBuilder::new(render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES);

        super::entity_iterator::process_archetype::<Self, Arrows3D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_space_view::RangeResultsExt as _;

                let resolver = ctx.recording().resolver();

                let vectors = match results.get_required_component_dense::<Vector3D>(resolver) {
                    Some(vectors) => vectors?,
                    _ => return Ok(()),
                };

                let num_vectors = vectors
                    .range_indexed()
                    .map(|(_, vectors)| vectors.len())
                    .sum::<usize>();
                if num_vectors == 0 {
                    return Ok(());
                }

                line_builder.reserve_strips(num_vectors)?;
                line_builder.reserve_vertices(num_vectors * 2)?;

                let origins = results.get_or_empty_dense(resolver)?;
                let colors = results.get_or_empty_dense(resolver)?;
                let radii = results.get_or_empty_dense(resolver)?;
                let labels = results.get_or_empty_dense(resolver)?;
                let class_ids = results.get_or_empty_dense(resolver)?;
                let keypoint_ids = results.get_or_empty_dense(resolver)?;

                let data = range_zip_1x6(
                    vectors.range_indexed(),
                    origins.range_indexed(),
                    colors.range_indexed(),
                    radii.range_indexed(),
                    labels.range_indexed(),
                    class_ids.range_indexed(),
                    keypoint_ids.range_indexed(),
                )
                .map(
                    |(_index, vectors, origins, colors, radii, labels, class_ids, keypoint_ids)| {
                        Arrows3DComponentData {
                            vectors,
                            origins: origins.unwrap_or_default(),
                            colors: colors.unwrap_or_default(),
                            radii: radii.unwrap_or_default(),
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids.unwrap_or_default(),
                            keypoint_ids: keypoint_ids.unwrap_or_default(),
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

impl TypedComponentFallbackProvider<Color> for Arrows3DVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

re_viewer_context::impl_component_fallback_provider!(Arrows3DVisualizer => [Color]);
