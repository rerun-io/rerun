use re_components::{
    ClassId, ColorRGBA, Component as _, InstanceKey, KeypointId, Label, Point3D, Radius,
};
use re_data_store::{EntityPath, InstancePathHash};
use re_log_types::ComponentName;
use re_query::{EntityView, QueryError};
use re_renderer::{LineStripSeriesBuilder, PointCloudBuilder};
use re_viewer_context::{ResolvedAnnotationInfo, SceneQuery, ViewerContext};

use crate::scene::{
    elements::{
        instance_key_to_picking_id, instance_path_hash_for_picking,
        process_annotations_and_keypoints, process_colors, process_radii, try_add_line_draw_data,
        try_add_point_draw_data,
    },
    load_keypoint_connections,
    spatial_scene_element::{SpatialSceneContext, SpatialSceneElement, SpatialSceneEntityContext},
    UiLabel, UiLabelTarget,
};

pub struct Points3DSceneElement {
    /// If the number of points in the batch is > max_labels, don't render point labels.
    pub max_labels: usize,
    pub ui_labels: Vec<UiLabel>,
}

impl Default for Points3DSceneElement {
    fn default() -> Self {
        Self {
            max_labels: 10,
            ui_labels: Vec::new(),
        }
    }
}

impl Points3DSceneElement {
    fn process_labels<'a>(
        entity_view: &'a EntityView<Point3D>,
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a [ResolvedAnnotationInfo],
        world_from_obj: glam::Affine3A,
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
                        target: UiLabelTarget::Position3D(
                            world_from_obj.transform_point3(point.into()),
                        ),
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
        entity_view: &EntityView<Point3D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
        point_builder: &mut PointCloudBuilder,
        line_builder: &mut re_renderer::LineStripSeriesBuilder,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let (annotation_infos, keypoints) =
            process_annotations_and_keypoints(query, entity_view, &ent_context.annotations)?;

        let colors = process_colors(entity_view, ent_path, &annotation_infos)?;
        let radii = process_radii(ent_path, entity_view)?;

        if entity_view.num_instances() <= self.max_labels {
            // Max labels is small enough that we can afford iterating on the colors again.
            let colors =
                process_colors(entity_view, ent_path, &annotation_infos)?.collect::<Vec<_>>();

            let instance_path_hashes_for_picking = {
                re_tracing::profile_scope!("instance_hashes");
                entity_view
                    .iter_instance_keys()
                    .map(|instance_key| {
                        instance_path_hash_for_picking(
                            ent_path,
                            instance_key,
                            entity_view.num_instances(),
                            ent_context.highlight.any_selection_highlight,
                        )
                    })
                    .collect::<Vec<_>>()
            };

            self.ui_labels.extend(Self::process_labels(
                entity_view,
                &instance_path_hashes_for_picking,
                &colors,
                &annotation_infos,
                ent_context.world_from_obj,
            )?);
        }

        {
            let point_batch = point_builder
                .batch("3d points")
                .world_from_obj(ent_context.world_from_obj)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

            let point_positions = {
                re_tracing::profile_scope!("collect_points");
                entity_view
                    .iter_primary()?
                    .filter_map(|pt| pt.map(glam::Vec3::from))
            };

            let picking_instance_ids = entity_view.iter_instance_keys().map(|instance_key| {
                instance_key_to_picking_id(
                    instance_key,
                    entity_view.num_instances(),
                    ent_context.highlight.any_selection_highlight,
                )
            });
            let mut point_range_builder = point_batch.add_points(
                entity_view.num_instances(),
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
                    let highlighted_point_index = entity_view
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
        }

        load_keypoint_connections(line_builder, ent_path, keypoints, &ent_context.annotations);

        Ok(())
    }
}

impl SpatialSceneElement<7> for Points3DSceneElement {
    type Primary = Point3D;

    fn archetype() -> [ComponentName; 7] {
        [
            Point3D::name(),
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
        context: SpatialSceneContext<'_>,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_scope!("Points3DPart");

        let mut point_builder = PointCloudBuilder::new(ctx.render_ctx);
        let mut line_builder = LineStripSeriesBuilder::new(ctx.render_ctx);

        Self::for_each_entity_view(
            ctx,
            query,
            &context,
            context.depth_offsets.points,
            |ent_path, entity_view, ent_context| {
                self.process_entity_view(
                    query,
                    &entity_view,
                    ent_path,
                    ent_context,
                    &mut point_builder,
                    &mut line_builder,
                )
            },
        );

        let mut draw_data_list = Vec::new();
        try_add_point_draw_data(ctx.render_ctx, point_builder, &mut draw_data_list);
        try_add_line_draw_data(ctx.render_ctx, line_builder, &mut draw_data_list);
        draw_data_list
    }
}
