use itertools::Itertools as _;
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId, PointCloudBuilder};
use re_sdk_types::archetypes::Points2D;
use re_sdk_types::components::{ClassId, Color, KeypointId, Position2D, Radius, ShowLabels};
use re_sdk_types::{Archetype as _, ArrowString};
use re_view::{process_annotation_and_keypoint_slices, process_color_slice};
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
    typed_fallback_for,
};

use super::SpatialViewVisualizerData;
use super::utilities::{LabeledBatch, process_labels_2d};
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::view_kind::SpatialViewKind;
use crate::visualizers::{load_keypoint_connections, process_radius_slice};

// ---

pub struct Points2DVisualizer {
    pub data: SpatialViewVisualizerData,
}

impl Default for Points2DVisualizer {
    fn default() -> Self {
        Self {
            data: SpatialViewVisualizerData::new(Some(SpatialViewKind::TwoD)),
        }
    }
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Points2DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        point_builder: &mut PointCloudBuilder<'_>,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
        data: impl Iterator<Item = Points2DComponentData<'a>>,
    ) -> Result<(), ViewSystemExecutionError> {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let num_instances = data.positions.len();

            let positions = data
                .positions
                .iter()
                .map(|p| glam::vec3(p.x(), p.y(), 0.0))
                .collect_vec();

            let picking_ids = (0..num_instances)
                .map(|i| PickingLayerInstanceId(i as _))
                .collect_vec();

            let (annotation_infos, keypoints) = process_annotation_and_keypoint_slices(
                query.latest_at,
                num_instances,
                positions.iter().copied(),
                data.keypoint_ids,
                data.class_ids,
                &ent_context.annotations,
            );

            // Has not custom fallback for radius, so we use the default.
            // TODO(andreas): It would be nice to have this handle this fallback as part of the query.
            let radii =
                process_radius_slice(entity_path, num_instances, data.radii, Radius::default());
            let colors = process_color_slice(
                ctx,
                Points2D::descriptor_colors().component,
                num_instances,
                &annotation_infos,
                data.colors,
            );

            let world_from_obj = ent_context
                .transform_info
                .single_transform_required_for_entity(entity_path, Points2D::name())
                .as_affine3a();

            {
                let point_batch = point_builder
                    .batch(entity_path.to_string())
                    .depth_offset(ent_context.depth_offset)
                    .flags(
                        re_renderer::renderer::PointCloudBatchFlags::FLAG_DRAW_AS_CIRCLES
                            | re_renderer::renderer::PointCloudBatchFlags::FLAG_ENABLE_SHADING,
                    )
                    .world_from_obj(world_from_obj)
                    .outline_mask_ids(ent_context.highlight.overall)
                    .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

                let mut point_range_builder =
                    point_batch.add_points_2d(&positions, &radii, &colors, &picking_ids);

                // Determine if there's any sub-ranges that need extra highlighting.
                {
                    re_tracing::profile_scope!("marking additional highlight points");
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

            let obj_space_bounding_box = macaw::BoundingBox::from_points(positions.iter().copied());
            self.data
                .add_bounding_box(entity_path.hash(), obj_space_bounding_box, world_from_obj);

            load_keypoint_connections(
                line_builder,
                &ent_context.annotations,
                world_from_obj,
                entity_path,
                &keypoints,
            )?;

            self.data.ui_labels.extend(process_labels_2d(
                LabeledBatch {
                    entity_path,
                    num_instances,
                    overall_position: obj_space_bounding_box.center().truncate(),
                    instance_positions: data.positions.iter().map(|p| glam::vec2(p.x(), p.y())),
                    labels: &data.labels,
                    colors: &colors,
                    show_labels: data.show_labels.unwrap_or_else(|| {
                        typed_fallback_for(ctx, Points2D::descriptor_show_labels().component)
                    }),
                    annotation_infos: &annotation_infos,
                },
                world_from_obj,
            ));
        }

        Ok(())
    }
}

// ---

#[doc(hidden)] // Public for benchmarks
pub struct Points2DComponentData<'a> {
    // Point of views
    pub positions: &'a [Position2D],

    // Clamped to edge
    pub colors: &'a [Color],
    pub radii: &'a [Radius],
    pub labels: Vec<ArrowString>,
    pub keypoint_ids: &'a [KeypointId],
    pub class_ids: &'a [ClassId],

    // Non-repeated
    show_labels: Option<ShowLabels>,
}

impl IdentifiedViewSystem for Points2DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Points2D".into()
    }
}

impl VisualizerSystem for Points2DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Points2D>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        let mut output = VisualizerExecutionOutput::default();

        let mut point_builder = PointCloudBuilder::new(ctx.viewer_ctx.render_ctx());
        point_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
        );

        // We need lines from keypoints. The number of lines we'll have is harder to predict, so we'll
        // go with the dynamic allocation approach.
        let mut line_builder = LineDrawableBuilder::new(ctx.viewer_ctx.render_ctx());
        line_builder.radius_boost_in_ui_points_for_outlines(
            re_view::SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
        );

        use super::entity_iterator::process_archetype;
        process_archetype::<Self, Points2D, _>(
            ctx,
            view_query,
            context_systems,
            &mut output,
            self.data.preferred_view_kind,
            |ctx, spatial_ctx, results| {
                let all_positions =
                    results.iter_required(Points2D::descriptor_positions().component);
                if all_positions.is_empty() {
                    return Ok(());
                }

                let num_positions = all_positions
                    .chunks()
                    .iter()
                    .flat_map(|chunk| chunk.iter_slices::<[f32; 2]>())
                    .map(|points| points.len())
                    .sum();

                if num_positions == 0 {
                    return Ok(());
                }

                point_builder.reserve(num_positions)?;
                let all_colors = results.iter_optional(Points2D::descriptor_colors().component);
                let all_radii = results.iter_optional(Points2D::descriptor_radii().component);
                let all_labels = results.iter_optional(Points2D::descriptor_labels().component);
                let all_class_ids =
                    results.iter_optional(Points2D::descriptor_class_ids().component);
                let all_keypoint_ids =
                    results.iter_optional(Points2D::descriptor_keypoint_ids().component);
                let all_show_labels =
                    results.iter_optional(Points2D::descriptor_show_labels().component);

                let data = re_query::range_zip_1x6(
                    all_positions.slice::<[f32; 2]>(),
                    all_colors.slice::<u32>(),
                    all_radii.slice::<f32>(),
                    all_labels.slice::<String>(),
                    all_class_ids.slice::<u16>(),
                    all_keypoint_ids.slice::<u16>(),
                    all_show_labels.slice::<bool>(),
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
                        Points2DComponentData {
                            positions: bytemuck::cast_slice(positions),
                            colors: colors.map_or(&[], |colors| bytemuck::cast_slice(colors)),
                            radii: radii.map_or(&[], |radii| bytemuck::cast_slice(radii)),
                            labels: labels.unwrap_or_default(),
                            class_ids: class_ids
                                .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                            keypoint_ids: keypoint_ids
                                .map_or(&[], |keypoint_ids| bytemuck::cast_slice(keypoint_ids)),
                            show_labels: show_labels
                                .map(|b| !b.is_empty() && b.value(0))
                                .map(Into::into),
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

        Ok(output.with_draw_data([
            point_builder.into_draw_data()?.into(),
            line_builder.into_draw_data()?.into(),
        ]))
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }
}
