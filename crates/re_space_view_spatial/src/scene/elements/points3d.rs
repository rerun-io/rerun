use re_components::{
    ClassId, ColorRGBA, Component as _, InstanceKey, KeypointId, Label, Point3D, Radius,
};
use re_data_store::{EntityPath, InstancePathHash};
use re_log_types::ComponentName;
use re_query::{EntityView, QueryError};
use re_viewer_context::{ResolvedAnnotationInfo, SceneQuery, SpaceViewHighlights, ViewerContext};

use crate::scene::{
    contexts::{SpatialSceneContext, SpatialSceneEntityContext},
    elements::{
        instance_key_to_picking_id, instance_path_hash_for_picking,
        process_annotations_and_keypoints, process_colors, process_radii,
    },
    load_keypoint_connections,
    spatial_scene_part::{SpatialScenePart, SpatialScenePartData},
    UiLabel, UiLabelTarget,
};

pub struct Points3DScenePart {
    /// If the number of points in the batch is > max_labels, don't render point labels.
    pub max_labels: usize,
    pub data: SpatialScenePartData,
}

impl Default for Points3DScenePart {
    fn default() -> Self {
        Self {
            max_labels: 10,
            data: Default::default(),
        }
    }
}

impl Points3DScenePart {
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
        ent_view: &EntityView<Point3D>,
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
                    .map(|instance_key| {
                        instance_path_hash_for_picking(
                            ent_path,
                            instance_key,
                            ent_view.num_instances(),
                            ent_context.highlight.any_selection_highlight,
                        )
                    })
                    .collect::<Vec<_>>()
            };

            self.data.ui_labels.extend(Self::process_labels(
                ent_view,
                &instance_path_hashes_for_picking,
                &colors,
                &annotation_infos,
                ent_context.world_from_obj,
            )?);
        }

        {
            let mut point_builder = ent_context.shared_render_builders.points();
            let point_batch = point_builder
                .batch("3d points")
                .world_from_obj(ent_context.world_from_obj)
                .outline_mask_ids(ent_context.highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

            let point_positions = {
                re_tracing::profile_scope!("collect_points");
                ent_view
                    .iter_primary()?
                    .filter_map(|pt| pt.map(glam::Vec3::from))
            };

            let picking_instance_ids = ent_view.iter_instance_keys().map(|instance_key| {
                instance_key_to_picking_id(
                    instance_key,
                    ent_view.num_instances(),
                    ent_context.highlight.any_selection_highlight,
                )
            });
            let mut point_range_builder = point_batch.add_points(
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
        }

        load_keypoint_connections(ent_context, ent_path, keypoints);

        {
            re_tracing::profile_scope!("points3d.bounding_box");
            self.data.bounding_box = self.data.bounding_box.union(
                macaw::BoundingBox::from_points(
                    ent_view
                        .iter_primary()?
                        .filter_map(|pt| pt.map(|pt| pt.into())),
                )
                .transform_affine3(&ent_context.world_from_obj),
            );
        }

        Ok(())
    }
}

impl SpatialScenePart<7> for Points3DScenePart {
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
        context: &SpatialSceneContext,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_scope!("Points3DPart");

        Self::for_each_entity_view(
            ctx,
            query,
            context,
            highlights,
            context.depth_offsets.points,
            |ent_path, entity_view, ent_context| {
                self.process_entity_view(query, &entity_view, ent_path, ent_context)
            },
        );

        Vec::new() // TODO(andreas): Optionally return point & line draw data once SharedRenderBuilders is gone.
    }

    fn data(&self) -> &crate::scene::spatial_scene_part::SpatialScenePartData {
        &self.data
    }
}
