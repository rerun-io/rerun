use std::sync::Arc;

use ahash::{HashMap, HashMapExt};
use glam::Mat4;

use re_data_store::{EntityPath, EntityProperties, InstancePathHash};
use re_log_types::{
    component_types::{ClassId, ColorRGBA, InstanceKey, KeypointId, Label, Point3D, Radius},
    msg_bundle::Component,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::Size;

use crate::{
    misc::{SpaceViewHighlights, SpaceViewOutlineMasks, TransformCache, ViewerContext},
    ui::{
        annotations::ResolvedAnnotationInfo,
        scene::SceneQuery,
        view_spatial::{
            scene::{scene_part::instance_path_hash_for_picking, Keypoints},
            SceneSpatial, UiLabel, UiLabelTarget,
        },
        Annotations, DefaultColor,
    },
};

use super::ScenePart;

pub struct Points3DPart {
    /// If the number of points in the batch is > max_labels, don't render point labels.
    pub(crate) max_labels: usize,
}

impl Points3DPart {
    fn process_annotations(
        query: &SceneQuery<'_>,
        entity_view: &EntityView<Point3D>,
        annotations: &Arc<Annotations>,
    ) -> Result<(Vec<ResolvedAnnotationInfo>, Keypoints), QueryError> {
        crate::profile_function!();

        let mut keypoints: Keypoints = HashMap::new();

        // No need to process annotations if we don't have keypoints or class-ids
        if !entity_view.has_component::<KeypointId>() && !entity_view.has_component::<ClassId>() {
            let resolved_annotation = annotations.class_description(None).annotation_info();
            return Ok((
                vec![resolved_annotation; entity_view.num_instances()],
                keypoints,
            ));
        }

        let annotation_info = itertools::izip!(
            entity_view.iter_primary()?,
            entity_view.iter_component::<KeypointId>()?,
            entity_view.iter_component::<ClassId>()?,
        )
        .map(|(position, keypoint_id, class_id)| {
            let class_description = annotations.class_description(class_id);

            if let (Some(keypoint_id), Some(class_id), Some(position)) =
                (keypoint_id, class_id, position)
            {
                keypoints
                    .entry((class_id, query.latest_at.as_i64()))
                    .or_insert_with(Default::default)
                    .insert(keypoint_id, position.into());
                class_description.annotation_info_with_keypoint(keypoint_id)
            } else {
                class_description.annotation_info()
            }
        })
        .collect();

        Ok((annotation_info, keypoints))
    }

    fn process_colors<'a>(
        entity_view: &'a EntityView<Point3D>,
        ent_path: &'a EntityPath,
        annotation_infos: &'a [ResolvedAnnotationInfo],
    ) -> Result<impl Iterator<Item = egui::Color32> + 'a, QueryError> {
        crate::profile_function!();
        let default_color = DefaultColor::EntityPath(ent_path);

        let colors = itertools::izip!(
            annotation_infos.iter(),
            entity_view.iter_component::<ColorRGBA>()?,
        )
        .map(move |(annotation_info, color)| {
            annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color)
        });
        Ok(colors)
    }

    fn process_radii(
        entity_view: &EntityView<Point3D>,
    ) -> Result<impl Iterator<Item = Size> + '_, QueryError> {
        Ok(entity_view
            .iter_component::<Radius>()?
            .map(|radius| radius.map_or(Size::AUTO, |r| Size::new_scene(r.0))))
    }

    fn process_labels<'a>(
        entity_view: &'a EntityView<Point3D>,
        instance_path_hashes: &'a [InstancePathHash],
        colors: &'a [egui::Color32],
        annotation_infos: &'a [ResolvedAnnotationInfo],
        world_from_obj: Mat4,
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
        properties: &EntityProperties,
        entity_view: &EntityView<Point3D>,
        ent_path: &EntityPath,
        world_from_obj: Mat4,
        entity_highlight: &SpaceViewOutlineMasks,
    ) -> Result<(), QueryError> {
        crate::profile_function!();

        scene.num_logged_3d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let show_labels = true;

        let point_positions = {
            crate::profile_scope!("collect_points");
            entity_view
                .iter_primary()?
                .filter_map(|pt| pt.map(glam::Vec3::from))
        };

        let (annotation_infos, keypoints) =
            Self::process_annotations(query, entity_view, &annotations)?;
        let instance_path_hashes_for_picking = {
            crate::profile_scope!("instance_hashes");
            entity_view
                .iter_instance_keys()?
                .map(|instance_key| {
                    instance_path_hash_for_picking(
                        ent_path,
                        instance_key,
                        entity_view,
                        properties,
                        entity_highlight.any_selection_highlight,
                    )
                })
                .collect::<Vec<_>>()
        };

        let colors = Self::process_colors(entity_view, ent_path, &annotation_infos)?;
        let radii = Self::process_radii(entity_view)?;

        if show_labels && instance_path_hashes_for_picking.len() <= self.max_labels {
            // Max labels is small enough that we can afford iterating on the colors again.
            let colors =
                Self::process_colors(entity_view, ent_path, &annotation_infos)?.collect::<Vec<_>>();

            scene.ui.labels.extend(Self::process_labels(
                entity_view,
                &instance_path_hashes_for_picking,
                &colors,
                &annotation_infos,
                world_from_obj,
            )?);
        }

        {
            let mut point_batch = scene
                .primitives
                .points
                .batch("3d points")
                .world_from_obj(world_from_obj)
                .outline_mask_ids(entity_highlight.overall);
            let mut point_range_builder = point_batch
                .add_points(entity_view.num_instances(), point_positions)
                .colors(colors)
                .radii(radii)
                .user_data(instance_path_hashes_for_picking.into_iter());

            // Determine if there's any subranges that need extra highlighting.
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

        scene.load_keypoint_connections(ent_path, keypoints, &annotations, properties.interactive);

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
                        &props,
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
