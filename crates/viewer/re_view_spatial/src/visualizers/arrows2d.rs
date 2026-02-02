use re_log_types::Instance;
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId, renderer::LineStripFlags};
use re_sdk_types::{
    Archetype as _, ArrowString,
    archetypes::Arrows2D,
    components::{ClassId, Color, Position2D, Radius, ShowLabels, Vector2D},
};
use re_view::{clamped_or, process_annotation_and_keypoint_slices, process_color_slice};
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

pub struct Arrows2DVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for Arrows2DVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::TwoD)),
        }
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Arrows2DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        line_builder: &mut re_renderer::LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        data: impl Iterator<Item = Arrows2DComponentData<'a>>,
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
                &[], // No keypoint ids.
                data.class_ids,
                &ent_context.annotations,
            );

            // Has not custom fallback for radius, so we use the default.
            // TODO(andreas): It would be nice to have this handle this fallback as part of the query.
            let radii =
                process_radius_slice(entity_path, num_instances, data.radii, Radius::default());
            let colors = process_color_slice(
                ctx,
                Arrows2D::descriptor_colors().component,
                num_instances,
                &annotation_infos,
                data.colors,
            );

            let world_from_obj = ent_context
                .transform_info
                .single_transform_required_for_entity(entity_path, Arrows2D::name())
                .as_affine3a();

            let mut line_batch = line_builder
                .batch(entity_path.to_string())
                .depth_offset(ent_context.depth_offset)
                .world_from_obj(world_from_obj)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

            let mut obj_space_bounding_box = macaw::BoundingBox::nothing();

            let origins = clamped_or(data.origins, &Position2D::ZERO);

            for (i, (vector, origin, radius, &color)) in
                itertools::izip!(data.vectors, origins, radii, &colors).enumerate()
            {
                let vector: glam::Vec2 = vector.0.into();
                let origin: glam::Vec2 = origin.0.into();
                let end = origin + vector;

                let segment = line_batch
                    .add_segment_2d(origin, end)
                    .radius(radius)
                    .color(color)
                    .flags(
                        LineStripFlags::FLAG_CAP_END_TRIANGLE
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

                obj_space_bounding_box.extend(origin.extend(0.0));
                obj_space_bounding_box.extend(end.extend(0.0));
            }

            self.data
                .add_bounding_box(entity_path.hash(), obj_space_bounding_box, world_from_obj);

            self.data.ui_labels.extend(process_labels_2d(
                LabeledBatch {
                    entity_path,
                    num_instances,
                    overall_position: obj_space_bounding_box.center().truncate(),
                    instance_positions: {
                        // Take middle point of every arrow.
                        let origins = clamped_or(data.origins, &Position2D::ZERO);
                        itertools::izip!(data.vectors, origins).map(|(vector, origin)| {
                            // `0.45` rather than `0.5` to account for cap and such
                            glam::Vec2::from(origin.0) + glam::Vec2::from(vector.0) * 0.45
                        })
                    },
                    labels: &data.labels,
                    colors: &colors,
                    show_labels: data.show_labels.unwrap_or_else(|| {
                        typed_fallback_for(ctx, Arrows2D::descriptor_show_labels().component)
                    }),
                    annotation_infos: &annotation_infos,
                },
                world_from_obj,
            ));
        }
    }
}

// ---

struct Arrows2DComponentData<'a> {
    // Point of views
    vectors: &'a [Vector2D],

    // Clamped to edge
    origins: &'a [Position2D],
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: Vec<ArrowString>,
    class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
}

impl IdentifiedViewSystem for Arrows2DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Arrows2D".into()
    }
}

impl VisualizerSystem for Arrows2DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Arrows2D>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let mut output = VisualizerExecutionOutput::default();

        let mut line_builder = LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES,
        );

        use super::entity_iterator::{iter_slices, process_archetype};
        process_archetype::<Self, Arrows2D, _>(
            ctx,
            view_query,
            context_systems,
            &mut output,
            self.data.preferred_view_kind,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let all_vector_chunks = results
                    .get_required_chunk(Arrows2D::descriptor_vectors().component)
                    .ensure_required(|err| spatial_ctx.report_error(err));
                if all_vector_chunks.is_empty() {
                    return Ok(());
                }

                let num_vectors = all_vector_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<[f32; 2]>())
                    .map(|vectors| vectors.len())
                    .sum();

                if num_vectors == 0 {
                    return Ok(());
                }

                line_builder.reserve_strips(num_vectors)?;
                line_builder.reserve_vertices(num_vectors * 2)?;

                let timeline = ctx.query.timeline();
                let all_vectors_indexed = iter_slices::<[f32; 2]>(&all_vector_chunks, timeline);
                let all_origins = results.iter_as(
                    |err| spatial_ctx.report_warning(err),
                    timeline,
                    Arrows2D::descriptor_origins().component,
                );
                let all_colors = results.iter_as(
                    |err| spatial_ctx.report_warning(err),
                    timeline,
                    Arrows2D::descriptor_colors().component,
                );
                let all_radii = results.iter_as(
                    |err| spatial_ctx.report_warning(err),
                    timeline,
                    Arrows2D::descriptor_radii().component,
                );
                let all_labels = results.iter_as(
                    |err| spatial_ctx.report_warning(err),
                    timeline,
                    Arrows2D::descriptor_labels().component,
                );
                let all_class_ids = results.iter_as(
                    |err| spatial_ctx.report_warning(err),
                    timeline,
                    Arrows2D::descriptor_class_ids().component,
                );
                let all_show_labels = results.iter_as(
                    |err| spatial_ctx.report_warning(err),
                    timeline,
                    Arrows2D::descriptor_show_labels().component,
                );

                let data = re_query::range_zip_1x6(
                    all_vectors_indexed,
                    all_origins.slice::<[f32; 2]>(),
                    all_colors.slice::<u32>(),
                    all_radii.slice::<f32>(),
                    all_labels.slice::<String>(),
                    all_class_ids.slice::<u16>(),
                    all_show_labels.slice::<bool>(),
                )
                .map(
                    |(_index, vectors, origins, colors, radii, labels, class_ids, show_labels)| {
                        Arrows2DComponentData {
                            vectors: bytemuck::cast_slice(vectors),
                            origins: origins.map_or(&[], |origins| bytemuck::cast_slice(origins)),
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
