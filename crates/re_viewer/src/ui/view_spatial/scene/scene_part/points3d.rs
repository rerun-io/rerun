use std::sync::Arc;

use ahash::{HashMap, HashMapExt};
use glam::{Mat4, Vec3};

use re_arrow_store::LatestAtQuery;
use re_data_store::{query::visit_type_data_5, FieldName, InstanceIdHash, ObjPath, ObjectProps};
use re_log_types::{
    field_types::{ClassId, ColorRGBA, KeypointId, Label, Point3D, Radius},
    msg_bundle::Component,
    IndexHash, MsgId, ObjectType,
};
use re_query::{query_entity_with_primary, EntityView, QueryError};
use re_renderer::Size;

use crate::{
    misc::ViewerContext,
    ui::{
        annotations::ResolvedAnnotationInfo,
        scene::SceneQuery,
        transform_cache::ReferenceFromObjTransform,
        view_spatial::{
            scene::{instance_hash_if_interactive, to_ecolor, Keypoints},
            Label3D, SceneSpatial,
        },
        Annotations, DefaultColor,
    },
};

use super::ScenePart;

pub struct Points3DPartClassic;

impl ScenePart for Points3DPartClassic {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut crate::misc::ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &crate::ui::transform_cache::TransformCache,
    ) {
        crate::profile_scope!("Points3DPartClassic");

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Point3D])
        {
            scene.num_logged_3d_objects += 1;

            let mut batch_size = 0;
            let mut show_labels = true;
            let mut label_batch = Vec::new();

            // If keypoints ids show up we may need to connect them later!
            // We include time in the key, so that the "Visible history" (time range queries) feature works.
            let mut keypoints: Keypoints = Default::default();

            let annotations = scene.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);
            let properties = query.obj_props.get(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

            let highlighted_paths = ctx.hovered().check_obj_path(obj_path.hash());

            let mut point_batch = scene
                .primitives
                .points
                .batch("3d points")
                .world_from_obj(world_from_obj);

            let visitor = |instance_index: Option<&IndexHash>,
                           time: i64,
                           _msg_id: &MsgId,
                           pos: &[f32; 3],
                           color: Option<&[u8; 4]>,
                           radius: Option<&f32>,
                           label: Option<&String>,
                           class_id: Option<&i32>,
                           keypoint_id: Option<&i32>| {
                batch_size += 1;

                let position = Vec3::from_slice(pos);

                let instance_hash =
                    instance_hash_if_interactive(obj_path, instance_index, properties.interactive);

                let class_id = class_id.map(|i| ClassId(*i as _));
                let class_description = annotations.class_description(class_id);

                let annotation_info = if let Some(keypoint_id) = keypoint_id {
                    let keypoint_id = KeypointId(*keypoint_id as _);
                    if let Some(class_id) = class_id {
                        keypoints
                            .entry((class_id, time))
                            .or_insert_with(Default::default)
                            .insert(keypoint_id, position);
                    }

                    class_description.annotation_info_with_keypoint(keypoint_id)
                } else {
                    class_description.annotation_info()
                };

                let mut color = to_ecolor(annotation_info.color(color, default_color));
                let mut radius = radius.copied().map_or(Size::AUTO, Size::new_scene);

                if highlighted_paths.contains_index(instance_hash.instance_index_hash) {
                    color = SceneSpatial::HOVER_COLOR;
                    radius = SceneSpatial::hover_size_boost(radius);
                }

                show_labels = batch_size < 10;
                if show_labels {
                    if let Some(label) = annotation_info.label(label) {
                        label_batch.push(Label3D {
                            text: label,
                            origin: world_from_obj.transform_point3(position),
                        });
                    }
                }

                point_batch
                    .add_point(position)
                    .radius(radius)
                    .color(color)
                    .user_data(instance_hash);
            };

            visit_type_data_5(
                obj_store,
                &FieldName::from("pos"),
                &time_query,
                ("color", "radius", "label", "class_id", "keypoint_id"),
                visitor,
            );

            if show_labels {
                scene.ui.labels_3d.extend(label_batch);
            }

            scene.load_keypoint_connections(
                obj_path,
                keypoints,
                &annotations,
                properties.interactive,
            );
        }
    }
}

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
        highlighted: &'a [bool],
        annotation_infos: &'a [ResolvedAnnotationInfo],
    ) -> Result<impl Iterator<Item = egui::Color32> + 'a, QueryError> {
        let default_color = DefaultColor::ObjPath(ent_path);

        let colors = itertools::izip!(
            highlighted.iter(),
            annotation_infos.iter(),
            entity_view.iter_component::<ColorRGBA>()?,
        )
        .map(move |(highlighted, annotation_info, color)| {
            if *highlighted {
                SceneSpatial::HOVER_COLOR
            } else {
                to_ecolor(
                    annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color),
                )
            }
        });
        Ok(colors)
    }

    fn process_radii<'a>(
        entity_view: &'a EntityView<Point3D>,
        highlighted: &'a [bool],
    ) -> Result<impl Iterator<Item = Size> + 'a, QueryError> {
        let radii = itertools::izip!(highlighted.iter(), entity_view.iter_component::<Radius>()?,)
            .map(move |(highlighted, radius)| {
                let radius = radius.map_or(Size::AUTO, |radius| Size::new_scene(radius.0));
                if *highlighted {
                    SceneSpatial::hover_size_boost(radius)
                } else {
                    radius
                }
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
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        properties: &ObjectProps,
        entity_view: &EntityView<Point3D>,
        ent_path: &ObjPath,
        world_from_obj: Mat4,
    ) -> Result<(), QueryError> {
        scene.num_logged_3d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let show_labels = true;

        let mut point_batch = scene
            .primitives
            .points
            .batch("3d points")
            .world_from_obj(world_from_obj);

        let point_positions = entity_view
            .iter_primary()?
            .filter_map(|pt| pt.map(glam::Vec3::from))
            .collect::<Vec<_>>();

        let (annotation_infos, keypoints) = Self::process_annotations(
            query,
            entity_view,
            &annotations,
            point_positions.as_slice(),
        )?;

        let highlighted_paths = ctx.hovered().check_obj_path(ent_path.hash());

        let instance_hashes = entity_view
            .iter_instances()?
            .map(|instance| {
                if properties.interactive {
                    InstanceIdHash::from_path_and_arrow_instance(ent_path, &instance)
                } else {
                    InstanceIdHash::NONE
                }
            })
            .collect::<Vec<_>>();
        let highlighted = instance_hashes
            .iter()
            .map(|hash| highlighted_paths.contains_index(hash.instance_index_hash))
            .collect::<Vec<_>>();

        let colors = Self::process_colors(entity_view, ent_path, &highlighted, &annotation_infos)?;

        let radii = Self::process_radii(entity_view, &highlighted)?;
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
        ctx: &mut crate::misc::ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &crate::ui::transform_cache::TransformCache,
    ) {
        crate::profile_scope!("Points3DPart");

        for (ent_path, props) in query.iter_entities() {
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(ent_path) else {
                continue;
            };

            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            match query_entity_with_primary::<Point3D>(
                &ctx.log_db.obj_db.arrow_store,
                &timeline_query,
                ent_path,
                &[
                    ColorRGBA::name(),
                    Radius::name(),
                    Label::name(),
                    ClassId::name(),
                    KeypointId::name(),
                ],
            )
            .and_then(|entity_view| {
                self.process_entity_view(
                    scene,
                    ctx,
                    query,
                    &props,
                    &entity_view,
                    ent_path,
                    world_from_obj,
                )
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying '{:?}': {:?}", ent_path, err);
                }
            }
        }
    }
}
