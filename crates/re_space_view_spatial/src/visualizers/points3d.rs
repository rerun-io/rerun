use re_entity_db::{EntityPath, InstancePathHash};
use re_log_types::TimeInt;
use re_renderer::{LineDrawableBuilder, PickingLayerInstanceId, PointCloudBuilder};
use re_types::{
    archetypes::Points3D,
    components::{ClassId, Color, InstanceKey, KeypointId, Position3D, Radius, Text},
};
use re_viewer_context::{
    Annotations, ApplicableEntities, IdentifiedViewSystem, ResolvedAnnotationInfos,
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
    filter_visualizable_3d_entities, Keypoints, SpatialViewVisualizerData,
    SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES,
};

pub struct Points3DVisualizer {
    /// If the number of points in the batch is > max_labels, don't render point labels.
    pub max_labels: usize,
    pub data: SpatialViewVisualizerData,
}

impl Default for Points3DVisualizer {
    fn default() -> Self {
        Self {
            max_labels: 10,
            data: SpatialViewVisualizerData::new(Some(SpatialSpaceViewKind::ThreeD)),
        }
    }
}

impl Points3DVisualizer {
    fn process_labels<'a>(
        labels: &'a [Option<Text>],
        positions: &'a [glam::Vec3],
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a ResolvedAnnotationInfos,
        world_from_obj: glam::Affine3A,
    ) -> impl Iterator<Item = UiLabel> + 'a {
        re_tracing::profile_function!();
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
                        target: UiLabelTarget::Position3D(world_from_obj.transform_point3(*point)),
                        labeled_instance: *labeled_instance,
                    }),
                    _ => None,
                }
            },
        )
    }

    fn process_data(
        &mut self,
        point_builder: &mut PointCloudBuilder<'_>,
        line_builder: &mut LineDrawableBuilder<'_>,
        query: &ViewQuery<'_>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: &Points3DComponentData<'_>,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        let LoadedPoints {
            annotation_infos,
            keypoints,
            positions,
            radii,
            colors,
            picking_instance_ids,
        } = LoadedPoints::load(data, ent_path, query.latest_at, &ent_context.annotations);

        {
            re_tracing::profile_scope!("to_gpu");

            let point_batch = point_builder
                .batch(ent_path.to_string())
                .world_from_obj(ent_context.world_from_entity)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

            let mut point_range_builder =
                point_batch.add_points(&positions, &radii, &colors, &picking_instance_ids);

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
        }

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
                    ent_context.world_from_entity,
                ));
            }
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
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let num_points = super::entity_iterator::count_instances_in_archetype_views::<
            Points3DVisualizer,
            Points3D,
            8,
        >(ctx, query);

        if num_points == 0 {
            return Ok(Vec::new());
        }

        let mut point_builder = PointCloudBuilder::new(ctx.render_ctx);
        point_builder.reserve(num_points)?;
        point_builder
            .radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES);

        // We need lines from keypoints. The number of lines we'll have is harder to predict, so we'll go with the dynamic allocation approach.
        let mut line_builder = LineDrawableBuilder::new(ctx.render_ctx);
        line_builder
            .radius_boost_in_ui_points_for_outlines(SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES);

        super::entity_iterator::process_archetype_pov1_comp5::<
            Points3DVisualizer,
            Points3D,
            Position3D,
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
                let data = Points3DComponentData {
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
                    ent_path,
                    ent_context,
                    &data,
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
}

// ---

#[doc(hidden)] // Public for benchmarks
pub struct LoadedPoints {
    pub annotation_infos: ResolvedAnnotationInfos,
    pub keypoints: Keypoints,
    pub positions: Vec<glam::Vec3>,
    pub radii: Vec<re_renderer::Size>,
    pub colors: Vec<re_renderer::Color32>,
    pub picking_instance_ids: Vec<PickingLayerInstanceId>,
}

#[doc(hidden)] // Public for benchmarks
pub struct Points3DComponentData<'a> {
    pub instance_keys: &'a [InstanceKey],
    pub positions: &'a [Position3D],
    pub colors: Option<&'a [Option<Color>]>,
    pub radii: Option<&'a [Option<Radius>]>,
    pub labels: Option<&'a [Option<Text>]>,
    pub keypoint_ids: Option<&'a [Option<KeypointId>]>,
    pub class_ids: Option<&'a [Option<ClassId>]>,
}

impl LoadedPoints {
    #[inline]
    pub fn load(
        data: &Points3DComponentData<'_>,
        ent_path: &EntityPath,
        latest_at: TimeInt,
        annotations: &Annotations,
    ) -> Self {
        re_tracing::profile_function!();

        let (annotation_infos, keypoints) = process_annotation_and_keypoint_slices(
            latest_at,
            data.instance_keys,
            data.keypoint_ids,
            data.class_ids,
            data.positions.iter().map(|p| p.0.into()),
            annotations,
        );

        let (positions, radii, colors, picking_instance_ids) = join4(
            || Self::load_positions(data),
            || Self::load_radii(data, ent_path),
            || Self::load_colors(data, ent_path, &annotation_infos),
            || Self::load_picking_ids(data),
        );

        Self {
            annotation_infos,
            keypoints,
            positions,
            radii,
            colors,
            picking_instance_ids,
        }
    }

    #[inline]
    pub fn load_positions(
        Points3DComponentData { positions, .. }: &Points3DComponentData<'_>,
    ) -> Vec<glam::Vec3> {
        re_tracing::profile_function!();
        bytemuck::cast_slice(positions).to_vec()
    }

    #[inline]
    pub fn load_radii(
        &Points3DComponentData {
            positions, radii, ..
        }: &Points3DComponentData<'_>,
        ent_path: &EntityPath,
    ) -> Vec<re_renderer::Size> {
        crate::visualizers::process_radius_slice(radii, positions.len(), ent_path)
    }

    #[inline]
    pub fn load_colors(
        &Points3DComponentData { colors, .. }: &Points3DComponentData<'_>,
        ent_path: &EntityPath,
        annotation_infos: &ResolvedAnnotationInfos,
    ) -> Vec<re_renderer::Color32> {
        crate::visualizers::process_color_slice(colors, ent_path, annotation_infos)
    }

    #[inline]
    pub fn load_picking_ids(
        &Points3DComponentData { instance_keys, .. }: &Points3DComponentData<'_>,
    ) -> Vec<PickingLayerInstanceId> {
        re_tracing::profile_function!();
        bytemuck::cast_slice(instance_keys).to_vec()
    }
}

/// Run 4 things in parallel
fn join4<A: Send, B: Send, C: Send, D: Send>(
    a: impl FnOnce() -> A + Send,
    b: impl FnOnce() -> B + Send,
    c: impl FnOnce() -> C + Send,
    d: impl FnOnce() -> D + Send,
) -> (A, B, C, D) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        re_tracing::profile_function!();
        let ((a, b), (c, d)) = rayon::join(|| rayon::join(a, b), || rayon::join(c, d));
        (a, b, c, d)
    }

    #[cfg(target_arch = "wasm32")]
    {
        (a(), b(), c(), d())
    }
}
