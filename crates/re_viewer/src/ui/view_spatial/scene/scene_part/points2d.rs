use glam::Mat4;
use re_arrow_store::LatestAtQuery;
use re_data_store::{query::visit_type_data_5, FieldName, InstanceIdHash, ObjPath, ObjectProps};
use re_log_types::{
    field_types::{ClassId, ColorRGBA, Instance, KeypointId, Label, Point2D, Radius},
    msg_bundle::Component,
    IndexHash, MsgId, ObjectType,
};
use re_query::{query_entity_with_primary, EntityView, QueryError};
use re_renderer::Size;

use crate::{
    misc::ViewerContext,
    ui::{
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::{
            scene::{
                apply_hover_effect, instance_hash_if_interactive, paint_properties, Keypoints,
            },
            Label2D, Label2DTarget, SceneSpatial,
        },
        DefaultColor,
    },
};

use super::ScenePart;

pub struct Points2DPartClassic;

impl ScenePart for Points2DPartClassic {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        hovered_instance: re_data_store::InstanceIdHash,
    ) {
        crate::profile_scope!("Points2DPartClassic");

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Point2D])
        {
            scene.num_logged_2d_objects += 1;

            let mut label_batch = Vec::new();
            let max_num_labels = 10;

            let annotations = scene.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);
            let properties = query.obj_props.get(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

            // If keypoints ids show up we may need to connect them later!
            // We include time in the key, so that the "Visible history" (time range queries) feature works.
            let mut keypoints: Keypoints = Default::default();

            let mut point_batch = scene
                .primitives
                .points
                .batch("2d points")
                .world_from_obj(world_from_obj);

            let visitor = |instance_index: Option<&IndexHash>,
                           time: i64,
                           _msg_id: &MsgId,
                           pos: &[f32; 2],
                           color: Option<&[u8; 4]>,
                           radius: Option<&f32>,
                           label: Option<&String>,
                           class_id: Option<&i32>,
                           keypoint_id: Option<&i32>| {
                let instance_hash =
                    instance_hash_if_interactive(obj_path, instance_index, properties.interactive);
                let pos = glam::vec2(pos[0], pos[1]);

                let class_id = class_id.map(|i| ClassId(*i as _));
                let class_description = annotations.class_description(class_id);

                let annotation_info = if let Some(keypoint_id) = keypoint_id {
                    let keypoint_id = KeypointId(*keypoint_id as _);
                    if let Some(class_id) = class_id {
                        keypoints
                            .entry((class_id, time))
                            .or_insert_with(Default::default)
                            .insert(keypoint_id, pos.extend(0.0));
                    }

                    class_description.annotation_info_with_keypoint(keypoint_id)
                } else {
                    class_description.annotation_info()
                };
                let color = annotation_info.color(color, default_color);
                let label = annotation_info.label(label);

                let mut paint_props = paint_properties(color, radius);
                if instance_hash.is_some() && hovered_instance == instance_hash {
                    apply_hover_effect(&mut paint_props);
                }

                point_batch
                    .add_point_2d(pos)
                    .color(paint_props.fg_stroke.color)
                    .radius(Size::new_points(paint_props.fg_stroke.width * 0.5))
                    .user_data(instance_hash);

                if let Some(label) = label {
                    if label_batch.len() < max_num_labels {
                        label_batch.push(Label2D {
                            text: label,
                            color: paint_props.fg_stroke.color,
                            target: Label2DTarget::Point(egui::pos2(pos.x, pos.y)),
                            labled_instance: instance_hash,
                        });
                    }
                }
            };
            visit_type_data_5(
                obj_store,
                &FieldName::from("pos"),
                &time_query,
                ("color", "radius", "label", "class_id", "keypoint_id"),
                visitor,
            );

            // TODO(andreas): Make user configurable with this as the default.
            if label_batch.len() < max_num_labels {
                scene.ui.labels_2d.extend(label_batch.into_iter());
            }

            // Generate keypoint connections if any.
            scene.load_keypoint_connections(
                obj_path,
                keypoints,
                &annotations,
                properties.interactive,
            );
        }
    }
}

pub struct Points2DPart;

impl Points2DPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        scene: &mut SceneSpatial,
        _query: &SceneQuery<'_>,
        props: &ObjectProps,
        hovered_instance: InstanceIdHash,
        entity_view: &EntityView<Point2D>,
        ent_path: &ObjPath,
        world_from_obj: Mat4,
    ) -> Result<(), QueryError> {
        scene.num_logged_2d_objects += 1;

        let mut label_batch = Vec::new();
        let max_num_labels = 10;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::ObjPath(ent_path);

        // If keypoints ids show up we may need to connect them later!
        // We include time in the key, so that the "Visible history" (time range queries) feature works.
        let mut keypoints: Keypoints = Default::default();

        let mut point_batch = scene
            .primitives
            .points
            .batch("2d points")
            .world_from_obj(world_from_obj);

        let visitor = |instance: Instance,
                       pos: Point2D,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>,
                       label: Option<Label>,
                       class_id: Option<ClassId>,
                       keypoint_id: Option<KeypointId>| {
            let instance_hash = {
                if props.interactive {
                    InstanceIdHash::from_path_and_arrow_instance(ent_path, &instance)
                } else {
                    InstanceIdHash::NONE
                }
            };

            let pos: glam::Vec2 = pos.into();

            let class_description = annotations.class_description(class_id);

            let annotation_info = keypoint_id.map_or_else(
                || class_description.annotation_info(),
                |keypoint_id| {
                    if let Some(class_id) = class_id {
                        keypoints
                            .entry((class_id, 0))
                            .or_insert_with(Default::default)
                            .insert(keypoint_id, pos.extend(0.0));
                    }
                    class_description.annotation_info_with_keypoint(keypoint_id)
                },
            );

            let color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);
            let label = annotation_info.label(label.map(|l| l.0).as_ref());

            let mut paint_props = paint_properties(color, radius.map(|r| r.0).as_ref());
            if instance_hash.is_some() && hovered_instance == instance_hash {
                apply_hover_effect(&mut paint_props);
            }

            point_batch
                .add_point_2d(pos)
                .color(paint_props.fg_stroke.color)
                .radius(Size::new_points(paint_props.fg_stroke.width * 0.5))
                .user_data(instance_hash);

            if let Some(label) = label {
                if label_batch.len() < max_num_labels {
                    label_batch.push(Label2D {
                        text: label,
                        color: paint_props.fg_stroke.color,
                        target: Label2DTarget::Point(egui::pos2(pos.x, pos.y)),
                        labled_instance: instance_hash,
                    });
                }
            }
        };

        entity_view.visit6(visitor)?;

        if label_batch.len() < max_num_labels {
            scene.ui.labels_2d.extend(label_batch.into_iter());
        }

        // Generate keypoint connections if any.
        scene.load_keypoint_connections(ent_path, keypoints, &annotations, props.interactive);

        Ok(())
    }
}

impl ScenePart for Points2DPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        hovered_instance: re_data_store::InstanceIdHash,
    ) {
        crate::profile_scope!("Points2DPart");

        for (ent_path, props) in query.iter_entities() {
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(ent_path) else {
                continue;
            };

            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            match query_entity_with_primary::<Point2D>(
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
                Self::process_entity_view(
                    scene,
                    query,
                    &props,
                    hovered_instance,
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
