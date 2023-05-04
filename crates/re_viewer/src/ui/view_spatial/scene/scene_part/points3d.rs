use re_data_store::{EntityPath, InstancePathHash};
use re_log_types::{
    component_types::{ClassId, ColorRGBA, InstanceKey, KeypointId, Label, Point3D, Radius},
    Component,
};
use re_query::{query_primary_with_history, EntityView, QueryError};

use crate::{
    misc::{SpaceViewHighlights, SpaceViewOutlineMasks, TransformCache, ViewerContext},
    ui::{
        annotations::ResolvedAnnotationInfo,
        scene::SceneQuery,
        view_spatial::{
            scene::scene_part::{
                instance_key_to_picking_id, instance_path_hash_for_picking,
                process_annotations_and_keypoints, process_colors, process_radii,
            },
            SceneSpatial, UiLabel, UiLabelTarget,
        },
    },
};

use super::ScenePart;

pub struct Points3DPart {
    /// If the number of points in the batch is > max_labels, don't render point labels.
    pub(crate) max_labels: usize,
}

impl Points3DPart {
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

    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        &self,
        scene: &mut SceneSpatial,
        query: &SceneQuery<'_>,
        entity_view: &EntityView<Point3D>,
        ent_path: &EntityPath,
        world_from_obj: glam::Affine3A,
        entity_highlight: &SpaceViewOutlineMasks,
    ) -> Result<(), QueryError> {
        crate::profile_function!();

        scene.num_logged_3d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);

        let (annotation_infos, keypoints) =
            process_annotations_and_keypoints(query, entity_view, &annotations)?;

        let colors = process_colors(entity_view, ent_path, &annotation_infos)?;
        let radii = process_radii(ent_path, entity_view)?;

        if entity_view.num_instances() <= self.max_labels {
            // Max labels is small enough that we can afford iterating on the colors again.
            let colors =
                process_colors(entity_view, ent_path, &annotation_infos)?.collect::<Vec<_>>();

            let instance_path_hashes_for_picking = {
                crate::profile_scope!("instance_hashes");
                entity_view
                    .iter_instance_keys()?
                    .map(|instance_key| {
                        instance_path_hash_for_picking(
                            ent_path,
                            instance_key,
                            entity_view,
                            entity_highlight.any_selection_highlight,
                        )
                    })
                    .collect::<Vec<_>>()
            };

            scene.ui.labels.extend(Self::process_labels(
                entity_view,
                &instance_path_hashes_for_picking,
                &colors,
                &annotation_infos,
                world_from_obj,
            )?);
        }

        {
            let point_batch = scene
                .primitives
                .points
                .batch("3d points")
                .world_from_obj(world_from_obj)
                .outline_mask_ids(entity_highlight.overall)
                .picking_object_id(re_renderer::PickingLayerObjectId(ent_path.hash64()));

            let point_positions = {
                crate::profile_scope!("collect_points");
                entity_view
                    .iter_primary()?
                    .filter_map(|pt| pt.map(glam::Vec3::from))
            };

            let picking_instance_ids = entity_view.iter_instance_keys()?.map(|instance_key| {
                instance_key_to_picking_id(
                    instance_key,
                    entity_view,
                    entity_highlight.any_selection_highlight,
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
                crate::profile_scope!("marking additional highlight points");
                for (highlighted_key, instance_mask_ids) in &entity_highlight.instances {
                    // TODO(andreas/jeremy): We can do this much more efficiently
                    let highlighted_point_index = entity_view
                        .iter_instance_keys()?
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

        scene.load_keypoint_connections(ent_path, keypoints, &annotations);

        Ok(())
    }
}

impl ScenePart for Points3DPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        highlights: &SpaceViewHighlights,
    ) {
        crate::profile_scope!("Points3DPart");

        for (ent_path, props) in query.iter_entities() {
            let Some(world_from_obj) = transforms.reference_from_entity(ent_path) else {
                continue;
            };
            let entity_highlight = highlights.entity_outline_mask(ent_path.hash());

            match query_primary_with_history::<Point3D, 7>(
                &ctx.log_db.entity_db.data_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [
                    Point3D::name(),
                    InstanceKey::name(),
                    ColorRGBA::name(),
                    Radius::name(),
                    Label::name(),
                    ClassId::name(),
                    KeypointId::name(),
                ],
            )
            .and_then(|entities| {
                for entity in entities {
                    self.process_entity_view(
                        scene,
                        query,
                        &entity,
                        ent_path,
                        world_from_obj,
                        entity_highlight,
                    )?;
                }
                Ok(())
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying {ent_path:?}: {err}");
                }
            }
        }
    }
}
