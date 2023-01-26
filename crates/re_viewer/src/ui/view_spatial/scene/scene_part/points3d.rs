use std::sync::Arc;

use ahash::{HashMap, HashMapExt};
use glam::{Mat4, Vec3};

use re_data_store::{InstanceIdHash, ObjPath, ObjectProps};
use re_log_types::{
    field_types::{ClassId, ColorRGBA, Instance, KeypointId, Label, Point3D, Radius},
    msg_bundle::Component,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_renderer::Size;

use crate::{
    misc::{
        InteractionHighlight, OptionalSpaceViewObjectHighlight, SpaceViewHighlights, ViewerContext,
    },
    ui::{
        annotations::ResolvedAnnotationInfo,
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::{scene::Keypoints, Label3D, SceneSpatial},
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
        point_positions: &[Vec3],
    ) -> Result<(Vec<ResolvedAnnotationInfo>, Keypoints), QueryError> {
        crate::profile_function!();

        let mut keypoints: Keypoints = HashMap::new();

        let annotation_info = itertools::izip!(
            point_positions.iter(),
            entity_view.iter_component::<KeypointId>()?,
            entity_view.iter_component::<ClassId>()?,
        )
        .map(|(position, keypoint_id, class_id)| {
            let class_description = annotations.class_description(class_id);

            if let Some(keypoint_id) = keypoint_id {
                if let Some(class_id) = class_id {
                    keypoints
                        .entry((class_id, query.latest_at.as_i64()))
                        .or_insert_with(Default::default)
                        .insert(keypoint_id, *position);
                }
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
        ent_path: &'a ObjPath,
        highlights: &'a [InteractionHighlight],
        annotation_infos: &'a [ResolvedAnnotationInfo],
    ) -> Result<impl Iterator<Item = egui::Color32> + 'a, QueryError> {
        crate::profile_function!();
        let default_color = DefaultColor::ObjPath(ent_path);

        let colors = itertools::izip!(
            highlights.iter(),
            annotation_infos.iter(),
            entity_view.iter_component::<ColorRGBA>()?,
        )
        .map(move |(highlight, annotation_info, color)| {
            SceneSpatial::apply_hover_and_selection_effect_color(
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color),
                *highlight,
            )
        });
        Ok(colors)
    }

    fn process_radii<'a>(
        entity_view: &'a EntityView<Point3D>,
        highlights: &'a [InteractionHighlight],
    ) -> Result<impl Iterator<Item = Size> + 'a, QueryError> {
        let radii = itertools::izip!(highlights.iter(), entity_view.iter_component::<Radius>()?,)
            .map(move |(highlight, radius)| {
                SceneSpatial::apply_hover_and_selection_effect_size(
                    radius.map_or(Size::AUTO, |radius| Size::new_scene(radius.0)),
                    *highlight,
                )
            });
        Ok(radii)
    }

    fn process_labels<'a>(
        entity_view: &'a EntityView<Point3D>,
        annotation_infos: &'a [ResolvedAnnotationInfo],
        world_from_obj: Mat4,
    ) -> Result<impl Iterator<Item = Label3D> + 'a, QueryError> {
        let labels = itertools::izip!(
            annotation_infos.iter(),
            entity_view.iter_primary()?,
            entity_view.iter_component::<Label>()?
        )
        .filter_map(move |(annotation_info, point, label)| {
            let label = annotation_info.label(label.map(|l| l.0).as_ref());
            match (point, label) {
                (Some(point), Some(label)) => Some(Label3D {
                    text: label,
                    origin: world_from_obj.transform_point3(point.into()),
                }),
                _ => None,
            }
        });
        Ok(labels)
    }

    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        &self,
        scene: &mut SceneSpatial,
        query: &SceneQuery<'_>,
        properties: &ObjectProps,
        entity_view: &EntityView<Point3D>,
        ent_path: &ObjPath,
        world_from_obj: Mat4,
        object_highlight: OptionalSpaceViewObjectHighlight<'_>,
    ) -> Result<(), QueryError> {
        crate::profile_function!();

        scene.num_logged_3d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let show_labels = true;

        let mut point_batch = scene
            .primitives
            .points
            .batch("3d points")
            .world_from_obj(world_from_obj);

        let point_positions = {
            crate::profile_scope!("collect_points");
            entity_view
                .iter_primary()?
                .filter_map(|pt| pt.map(glam::Vec3::from))
                .collect::<Vec<_>>()
        };

        let (annotation_infos, keypoints) = Self::process_annotations(
            query,
            entity_view,
            &annotations,
            point_positions.as_slice(),
        )?;

        let instance_hashes = {
            crate::profile_scope!("instance_hashes");
            entity_view
                .iter_instances()?
                .map(|instance| {
                    if properties.interactive {
                        InstanceIdHash::from_path_and_arrow_instance(ent_path, &instance)
                    } else {
                        InstanceIdHash::NONE
                    }
                })
                .collect::<Vec<_>>()
        };

        let highlights = {
            crate::profile_scope!("highlights");
            instance_hashes
                .iter()
                .map(|hash| object_highlight.index_highlight(hash.instance_index_hash))
                .collect::<Vec<_>>()
        };

        let colors = Self::process_colors(entity_view, ent_path, &highlights, &annotation_infos)?;

        let radii = Self::process_radii(entity_view, &highlights)?;
        let labels = Self::process_labels(entity_view, &annotation_infos, world_from_obj)?;

        if show_labels && instance_hashes.len() <= self.max_labels {
            scene.ui.labels_3d.extend(labels);
        }

        point_batch
            .add_points(point_positions.into_iter())
            .colors(colors)
            .radii(radii)
            .user_data(instance_hashes.into_iter());

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
            let Some(world_from_obj) = transforms.reference_from_obj(ent_path) else {
                continue;
            };
            let object_highlight = highlights.object_highlight(ent_path.hash());

            match query_primary_with_history::<Point3D, 7>(
                &ctx.log_db.obj_db.arrow_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [
                    Point3D::name(),
                    Instance::name(),
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
                        object_highlight,
                    )?;
                }
                Ok(())
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying '{:?}': {:?}", ent_path, err);
                }
            }
        }
    }
}
