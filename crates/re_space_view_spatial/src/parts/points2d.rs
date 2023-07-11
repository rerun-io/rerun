use re_components::{
    ClassId, ColorRGBA, Component, InstanceKey, KeypointId, Label, Point2D, Radius,
};
use re_data_store::{EntityPath, InstancePathHash};
use re_query::{EntityView, QueryError};
use re_viewer_context::{
    ArchetypeDefinition, ResolvedAnnotationInfo, SceneQuery, SpaceViewHighlights, ViewPartSystem,
    ViewerContext,
};

use crate::{
    contexts::{SpatialSceneEntityContext, SpatialViewContext},
    parts::{
        entity_iterator::process_entity_views, load_keypoint_connections, UiLabel, UiLabelTarget,
    },
    SpatialSpaceView,
};

use super::{
    picking_id_from_instance_key, process_annotations_and_keypoints, process_colors, process_radii,
    SpatialScenePartData, SpatialSpaceViewState,
};

pub struct Points2DPart {
    /// If the number of points in the batch is > max_labels, don't render point labels.
    pub max_labels: usize,
    pub data: SpatialScenePartData,
}

impl Default for Points2DPart {
    fn default() -> Self {
        Self {
            max_labels: 10,
            data: Default::default(),
        }
    }
}

impl Points2DPart {
    fn process_labels<'a>(
        entity_view: &'a EntityView<Point2D>,
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a [ResolvedAnnotationInfo],
    ) -> Result<impl Iterator<Item = UiLabel> + 'a, QueryError> {
        let labels = itertools::izip!(
            annotation_infos.iter(),
            entity_view.iter_primary()?,
            entity_view.iter_component::<Label>()?,
            colors,
            instance_path_hashes,
        )
        .filter_map(
            move |(annotation_info, point, label, color, labeled_instance)| {
                let label = annotation_info.label(label.map(|l| l.0).as_ref());
                match (point, label) {
                    (Some(point), Some(label)) => Some(UiLabel {
                        text: label,
                        color: *color,
                        target: UiLabelTarget::Point2D(egui::pos2(point.x, point.y)),
                        labeled_instance: *labeled_instance,
                    }),
                    _ => None,
                }
            },
        );
        Ok(labels)
    }

    fn process_entity_view(
        &mut self,
        query: &SceneQuery<'_>,
        ent_view: &EntityView<Point2D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let (annotation_infos, keypoints) =
            process_annotations_and_keypoints(query, ent_view, &ent_context.annotations)?;

        let colors = process_colors(ent_view, ent_path, &annotation_infos)?;
        let radii = process_radii(ent_path, ent_view)?;

        if ent_view.num_instances() <= self.max_labels {
            // Max labels is small enough that we can afford iterating on the colors again.
            let colors = process_colors(ent_view, ent_path, &annotation_infos)?.collect::<Vec<_>>();

            let instance_path_hashes_for_picking = {
                re_tracing::profile_scope!("instance_hashes");
                ent_view
                    .iter_instance_keys()
                    .map(|instance_key| InstancePathHash::instance(ent_path, instance_key))
                    .collect::<Vec<_>>()
            };

            self.data.ui_labels.extend(Self::process_labels(
                ent_view,
                &instance_path_hashes_for_picking,
                &colors,
                &annotation_infos,
            )?);
        }

        {
            let mut point_builder = ent_context.shared_render_builders.points();
            let point_batch = point_builder
                .batch("2d points")
                .depth_offset(ent_context.depth_offset)
                .flags(
                    re_renderer::renderer::PointCloudBatchFlags::FLAG_DRAW_AS_CIRCLES
                        | re_renderer::renderer::PointCloudBatchFlags::FLAG_ENABLE_SHADING,
                )
                .world_from_obj(ent_context.world_from_obj)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

            let point_positions = {
                re_tracing::profile_scope!("collect_points");
                ent_view
                    .iter_primary()?
                    .filter_map(|pt| pt.map(glam::Vec2::from))
            };

            let picking_instance_ids = ent_view
                .iter_instance_keys()
                .map(picking_id_from_instance_key);

            let mut point_range_builder = point_batch.add_points_2d(
                ent_view.num_instances(),
                point_positions,
                radii,
                colors,
                picking_instance_ids,
            );

            // Determine if there's any sub-ranges that need extra highlighting.
            {
                re_tracing::profile_scope!("marking additional highlight points");
                for (highlighted_key, instance_mask_ids) in &ent_context.highlight.instances {
                    // TODO(andreas/jeremy): We can do this much more efficiently
                    let highlighted_point_index = ent_view
                        .iter_instance_keys()
                        .position(|key| key == *highlighted_key);
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

        load_keypoint_connections(ent_context, ent_path, keypoints);

        self.data.extend_bounding_box_with_points(
            ent_view
                .iter_primary()?
                .filter_map(|pt| pt.map(|pt| glam::vec3(pt.x, pt.y, 0.0))),
            ent_context.world_from_obj,
        );

        Ok(())
    }
}

impl ViewPartSystem<SpatialSpaceView> for Points2DPart {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![
            Point2D::name(),
            InstanceKey::name(),
            ColorRGBA::name(),
            Radius::name(),
            Label::name(),
            ClassId::name(),
            KeypointId::name(),
        ]
    }

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        _space_view_state: &SpatialSpaceViewState,
        scene_context: &SpatialViewContext,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_scope!("Points2DPart");

        process_entity_views::<re_components::Point2D, 7, _>(
            ctx,
            query,
            scene_context,
            highlights,
            scene_context.depth_offsets.points,
            self.archetype(),
            |_ctx, ent_path, entity_view, ent_context| {
                self.process_entity_view(query, &entity_view, ent_path, ent_context)
            },
        );

        Vec::new() // TODO(andreas): Optionally return point & line draw data once SharedRenderBuilders is gone.
    }

    fn data(&self) -> Option<&SpatialScenePartData> {
        Some(&self.data)
    }
}
