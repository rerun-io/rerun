use re_entity_db::{EntityPath, InstancePathHash};
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId, PointCloudBuilder};
use re_types::{
    archetypes::Points2D,
    components::{ClassId, Color, InstanceKey, KeypointId, Position2D, Radius, Text},
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, ResolvedAnnotationInfos,
    SpaceViewSystemExecutionError, ViewContextCollection, ViewQuery, ViewerContext,
    VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    view_kind::SpatialSpaceViewKind,
    visualizers::{
        load_keypoint_connections, process_annotation_and_keypoint_slices, process_color_slice,
        UiLabel, UiLabelTarget,
    },
};

use super::{
    filter_visualizable_2d_entities, SpatialViewVisualizerData,
    SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
};

// ---

pub struct Points2DVisualizer {
    /// If the number of points in the batch is > max_labels, don't render point labels.
    pub max_labels: usize,
    pub data: SpatialViewVisualizerData,
}

impl Default for Points2DVisualizer {
    fn default() -> Self {
        Self {
            max_labels: 10,
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::TwoD)),
        }
    }
}

impl Points2DVisualizer {
    fn process_labels<'a>(
        labels: &'a [Option<Text>],
        positions: &'a [glam::Vec3],
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
    ) -> impl Iterator<Item = UiLabel> + 'a {
        itertools::izip!(
            annotation_infos.iter(),
            positions,
            labels,
            colors,
            instance_path_hashes,
        )
        .filter_map(
            move |(annotation_info, point, label, color, labeled_instance)| {
                let label = annotation_info.label(label.as_ref().map(|l| l.as_str()));
                match (point, label) {
                    (point, Some(label)) => Some(UiLabel {
                        text: label,
                        color: *color,
                        target: UiLabelTarget::Point2D(egui::pos2(point.x, point.y)),
                        labeled_instance: *labeled_instance,
                    }),
                    _ => None,
                }
            },
        )
    }

    fn process_data(
        &mut self,
        point_builder: &mut PointCloudBuilder,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        data: &Points2DComponentData<'_>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let (annotation_infos, keypoints) = process_annotation_and_keypoint_slices(
            query.latest_at,
            data.instance_keys,
            data.keypoint_ids,
            data.class_ids,
            data.positions.iter().map(|p| glam::vec3(p.x(), p.y(), 0.0)),
            &ent_context.annotations,
        );

        let positions = Self::load_positions(data);
        let colors = Self::load_colors(data, ent_path, &annotation_infos);
        let radii = Self::load_radii(data, ent_path);
        let picking_instance_ids = Self::load_picking_ids(data);

        {
            re_tracing::profile_scope!("to_gpu");

            let point_batch = point_builder
                .batch(ent_path.to_string())
                .depth_offset(ent_context.depth_offset)
                .flags(
                    re_renderer::renderer::PointCloudBatchFlags::FLAG_DRAW_AS_CIRCLES
                        | re_renderer::renderer::PointCloudBatchFlags::FLAG_ENABLE_SHADING,
                )
                .world_from_obj(ent_context.world_from_entity)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

            let mut point_range_builder =
                point_batch.add_points_2d(&positions, &radii, &colors, &picking_instance_ids);

            // Determine if there's any sub-ranges that need extra highlighting.
            {
                re_tracing::profile_scope!("marking additional highlight points");
                for (highlighted_key, instance_mask_ids) in &ent_context.highlight.instances {
                    // TODO(andreas, jeremy): We can do this much more efficiently
                    let highlighted_point_index = data
                        .instance_keys
                        .iter()
                        .position(|key| highlighted_key == key);
                    if let Some(highlighted_point_index) = highlighted_point_index {
                        point_range_builder = point_range_builder
                            .push_additional_outline_mask_ids_for_range(
                                highlighted_point_index as u32..highlighted_point_index as u32 + 1,
                                *instance_mask_ids,
                            );
                    }
                }
            }
        };

        self.data.add_bounding_box_from_points(
            ent_path.hash(),
            positions.iter().copied(),
            ent_context.world_from_entity,
        );

        load_keypoint_connections(line_builder, ent_context, ent_path, &keypoints)?;

        if data.instance_keys.len() <= self.max_labels {
            re_tracing::profile_scope!("labels");

            // Max labels is small enough that we can afford iterating on the colors again.
            let colors = process_color_slice(data.colors, ent_path, &annotation_infos);

            let instance_path_hashes_for_picking = {
                re_tracing::profile_scope!("instance_hashes");
                data.instance_keys
                    .iter()
                    .copied()
                    .map(|instance_key| InstancePathHash::instance(ent_path, instance_key))
                    .collect::<Vec<_>>()
            };

            if let Some(labels) = data.labels {
                self.data.ui_labels.extend(Self::process_labels(
                    labels,
                    &positions,
                    &instance_path_hashes_for_picking,
                    &colors,
                    &annotation_infos,
                ));
            }
        }

        Ok(())
    }

    #[inline]
    pub fn load_positions(
        Points2DComponentData { positions, .. }: &Points2DComponentData<'_>,
    ) -> Vec<glam::Vec3> {
        re_tracing::profile_function!();
        positions
            .iter()
            .map(|p| glam::vec3(p.x(), p.y(), 0.0))
            .collect()
    }

    #[inline]
    pub fn load_radii(
        &Points2DComponentData {
            positions, radii, ..
        }: &Points2DComponentData<'_>,
        ent_path: &EntityPath,
    ) -> Vec<re_renderer::Size> {
        crate::visualizers::process_radius_slice(radii, positions.len(), ent_path)
    }

    #[inline]
    pub fn load_colors(
        &Points2DComponentData { colors, .. }: &Points2DComponentData<'_>,
        ent_path: &EntityPath,
        annotation_infos: &ResolvedAnnotationInfos,
    ) -> Vec<re_renderer::Color32> {
        crate::visualizers::process_color_slice(colors, ent_path, annotation_infos)
    }

    #[inline]
    pub fn load_picking_ids(
        &Points2DComponentData { instance_keys, .. }: &Points2DComponentData<'_>,
    ) -> Vec<PickingLayerInstanceId> {
        re_tracing::profile_function!();
        bytemuck::cast_slice(instance_keys).to_vec()
    }
}

// ---

#[doc(hidden)] // Public for benchmarks
pub struct Points2DComponentData<'a> {
    pub instance_keys: &'a [InstanceKey],
    pub positions: &'a [Position2D],
    pub colors: Option<&'a [Option<Color>]>,
    pub radii: Option<&'a [Option<Radius>]>,
    pub labels: Option<&'a [Option<Text>]>,
    pub keypoint_ids: Option<&'a [Option<KeypointId>]>,
    pub class_ids: Option<&'a [Option<ClassId>]>,
}

impl IdentifiedViewSystem for Points2DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Points2D".into()
    }
}

impl VisualizerSystem for Points2DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Points2D>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_2d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let num_points = super::entity_iterator::count_instances_in_archetype_views::<
            Points2DVisualizer,
            Points2D,
            9,
        >(ctx, query);

        if num_points == 0 {
            return Ok(Vec::new());
        }

        let mut point_builder = PointCloudBuilder::new(ctx.render_ctx, num_points)
            .radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES);

        // We need lines from keypoints. The number of lines we'll have is harder to predict, so we'll go with the dynamic allocation approach.
        let mut line_builder = LineDrawableBuilder::new(ctx.render_ctx);
        line_builder
            .radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES);

        super::entity_iterator::process_archetype_pov1_comp5::<
            Points2DVisualizer,
            Points2D,
            Position2D,
            Color,
            Radius,
            Text,
            re_types::components::KeypointId,
            re_types::components::ClassId,
            _,
        >(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            |_ctx,
             ent_path,
             _ent_props,
             ent_context,
             (_time, _row_id),
             instance_keys,
             positions,
             colors,
             radii,
             labels,
             keypoint_ids,
             class_ids| {
                let data = Points2DComponentData {
                    instance_keys,
                    positions,
                    colors,
                    radii,
                    labels,
                    keypoint_ids,
                    class_ids,
                };
                self.process_data(
                    &mut point_builder,
                    &mut line_builder,
                    query,
                    &data,
                    ent_path,
                    ent_context,
                )
            },
        )?;

        Ok(vec![
            point_builder.into_draw_data(ctx.render_ctx)?.into(),
            line_builder.into_draw_data()?.into(),
        ])
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.data.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
