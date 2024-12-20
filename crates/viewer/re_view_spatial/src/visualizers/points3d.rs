use itertools::Itertools;

use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId, PointCloudBuilder};
use re_types::{
    archetypes::Points3D,
    components::{ClassId, Color, KeypointId, Position3D, Radius, ShowLabels, Text},
    ArrowString, Component,
};
use re_view::{process_annotation_and_keypoint_slices, process_color_slice};
use re_viewer_context::{
    auto_color_for_entity_path, ApplicableEntities, IdentifiedViewSystem, QueryContext,
    TypedComponentFallbackProvider, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::{
    contexts::SpatialSceneEntityContext,
    view_kind::SpatialViewKind,
    visualizers::{load_keypoint_connections, process_radius_slice},
};

use super::{
    filter_visualizable_3d_entities, process_labels_3d, utilities::LabeledBatch,
    SpatialViewVisualizerData,
};

// ---

pub struct Points3DVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for Points3DVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::ThreeD)),
        }
    }
}

struct Points3DComponentData<'a> {
    // Point of views
    positions: &'a [Position3D],

    // Clamped to edge
    colors: &'a [Color],
    radii: &'a [Radius],
    labels: Vec<ArrowString>,
    keypoint_ids: &'a [KeypointId],
    class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Points3DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        point_builder: &mut PointCloudBuilder<'_>,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Points3DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let num_instances = data.positions.len();
            if num_instances == 0 {
                continue;
            }

            let picking_ids = (0..num_instances)
                .map(|i| PickingLayerInstanceId(i as _))
                .collect_vec();

            let (annotation_infos, keypoints) = process_annotation_and_keypoint_slices(
                query.latest_at,
                num_instances,
                data.positions.iter().map(|p| p.0.into()),
                data.keypoint_ids,
                data.class_ids,
                &ent_context.annotations,
            );

            let positions = bytemuck::cast_slice(data.positions);

            // Has not custom fallback for radius, so we use the default.
            // TODO(andreas): It would be nice to have this handle this fallback as part of the query.
            let radii =
                process_radius_slice(entity_path, num_instances, data.radii, Radius::default());
            let colors =
                process_color_slice(ctx, self, num_instances, &annotation_infos, data.colors);

            let world_from_obj = ent_context
                .transform_info
                .single_entity_transform_required(entity_path, "Points3D");

            {
                let point_batch = point_builder
                    .batch(entity_path.to_string())
                    .world_from_obj(world_from_obj)
                    .outline_mask_ids(ent_context.highlight.overall)
                    .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

                let mut point_range_builder =
                    point_batch.add_points(positions, &radii, &colors, &picking_ids);

                // Determine if there's any sub-ranges that need extra highlighting.
                {
                    for (highlighted_key, instance_mask_ids) in &ent_context.highlight.instances {
                        let highlighted_point_index = (highlighted_key.get()
                            < num_instances as u64)
                            .then_some(highlighted_key.get());
                        if let Some(highlighted_point_index) = highlighted_point_index {
                            point_range_builder = point_range_builder
                                .push_additional_outline_mask_ids_for_range(
                                    highlighted_point_index as u32
                                        ..highlighted_point_index as u32 + 1,
                                    *instance_mask_ids,
                                );
                        }
                    }
                }
            }

            let obj_space_bounding_box =
                re_math::BoundingBox::from_points(positions.iter().copied());
            self.data
                .add_bounding_box(entity_path.hash(), obj_space_bounding_box, world_from_obj);

            load_keypoint_connections(line_builder, ent_context, entity_path, &keypoints)?;

            self.data.ui_labels.extend(process_labels_3d(
                LabeledBatch {
                    entity_path,
                    num_instances,
                    overall_position: obj_space_bounding_box.center(),
                    instance_positions: positions.iter().copied(),
                    labels: &data.labels,
                    colors: &colors,
                    show_labels: data.show_labels.unwrap_or_else(|| self.fallback_for(ctx)),
                    annotation_infos: &annotation_infos,
                },
                world_from_obj,
            ));
        }

        Ok(())
    }
}

impl IdentifiedViewSystem for Points3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Points3D".into()
    }
}

impl VisualizerSystem for Points3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Points3D>()
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
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return Err(ViewSystemExecutionError::NoRenderContextError);
        };

        let mut point_builder = PointCloudBuilder::new(render_ctx);
        point_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
        );

        // We need lines from keypoints. The number of lines we'll have is harder to predict, so we'll go
        // with the dynamic allocation approach.
        let mut line_builder = LineDrawableBuilder::new(render_ctx);
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
        );

        use super::entity_iterator::{iter_primitive_array, process_archetype};
        process_archetype::<Self, Points3D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let Some(all_position_chunks) = results.get_required_chunks(&Position3D::name())
                else {
                    return Ok(());
                };

                let num_positions = all_position_chunks
                    .iter()
                    .flat_map(|chunk| chunk.iter_primitive_array::<3, f32>(&Position3D::name()))
                    .map(|points| points.len())
                    .sum();

                if num_positions == 0 {
                    return Ok(());
                }

                point_builder.reserve(num_positions)?;

                let timeline = ctx.query.timeline();
                let all_positions_indexed = iter_primitive_array::<3, f32>(
                    &all_position_chunks,
                    timeline,
                    Position3D::name(),
                );
                let all_colors = results.iter_as(timeline, Color::name());
                let all_radii = results.iter_as(timeline, Radius::name());
                let all_labels = results.iter_as(timeline, Text::name());
                let all_class_ids = results.iter_as(timeline, ClassId::name());
                let all_keypoint_ids = results.iter_as(timeline, KeypointId::name());
                let all_show_labels = results.iter_as(timeline, ShowLabels::name());

                let data = re_query::range_zip_1x6(
                    all_positions_indexed,
                    all_colors.primitive::<u32>(),
                    all_radii.primitive::<f32>(),
                    all_labels.string(),
                    all_class_ids.primitive::<u16>(),
                    all_keypoint_ids.primitive::<u16>(),
                    all_show_labels.bool(),
                )
                .map(
                    |(
                        _index,
                        positions,
                        colors,
                        radii,
                        labels,
                        class_ids,
                        keypoint_ids,
                        show_labels,
                    )| {
                        Points3DComponentData {
                            positions: bytemuck::cast_slice(positions),
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            radii: radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids
                                .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                            keypoint_ids: keypoint_ids
                                .map_or(&[], |keypoint_ids| bytemuck::cast_slice(keypoint_ids)),
                            show_labels: show_labels.unwrap_or_default().get(0).map(Into::into),
                        }
                    },
                );

                self.process_data(
                    ctx,
                    &mut point_builder,
                    &mut line_builder,
                    view_query,
                    spatial_ctx,
                    data,
                )
            },
        )?;

        Ok(vec![
            point_builder.into_draw_data()?.into(),
            line_builder.into_draw_data()?.into(),
        ])
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

impl TypedComponentFallbackProvider<Color> for Points3DVisualizer {
    #[inline]
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> Color {
        auto_color_for_entity_path(ctx.target_entity_path)
    }
}

impl TypedComponentFallbackProvider<ShowLabels> for Points3DVisualizer {
    fn fallback_for(&self, ctx: &QueryContext<'_>) -> ShowLabels {
        super::utilities::show_labels_fallback::<Position3D>(ctx)
    }
}

re_viewer_context::impl_component_fallback_provider!(Points3DVisualizer => [Color, ShowLabels]);
