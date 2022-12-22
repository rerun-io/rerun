use ahash::HashMap;
use glam::Vec3;
use re_data_store::{query::visit_type_data_5, FieldName};
use re_log_types::{
    context::{ClassId, KeypointId},
    IndexHash, MsgId, ObjectType,
};
use re_renderer::Size;

use crate::ui::{
    transform_cache::ReferenceFromObjTransform,
    view_spatial::{
        scene::{instance_hash_if_interactive, to_ecolor},
        Label3D, SceneSpatial,
    },
    DefaultColor,
};

use super::ScenePart;

pub struct Points3DPart;

impl ScenePart for Points3DPart {
    fn load(
        scene: &mut crate::ui::view_spatial::SceneSpatial,
        ctx: &mut crate::misc::ViewerContext<'_>,
        query: &crate::ui::scene::SceneQuery<'_>,
        transforms: &crate::ui::transform_cache::TransformCache,
        objects_properties: &re_data_store::ObjectsProperties,
        hovered_instance: re_data_store::InstanceIdHash,
    ) {
        crate::profile_function!();

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Point3D])
        {
            let mut batch_size = 0;
            let mut show_labels = true;
            let mut label_batch = Vec::new();

            // If keypoints ids show up we may need to connect them later!
            // We include time in the key, so that the "Visible history" (time range queries) feature works.
            let mut keypoints: HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec3>> =
                Default::default();

            let annotations = scene.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);
            let properties = objects_properties.get(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

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

                if instance_hash.is_some() && instance_hash == hovered_instance {
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
