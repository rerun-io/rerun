use ahash::HashMap;
use re_data_store::{query::visit_type_data_5, FieldName};
use re_log_types::{context::KeypointId, field_types::ClassId, IndexHash, MsgId, ObjectType};
use re_renderer::Size;

use crate::ui::{
    transform_cache::ReferenceFromObjTransform,
    view_spatial::{
        scene::{apply_hover_effect, instance_hash_if_interactive, paint_properties},
        Label2D, Label2DTarget,
    },
    DefaultColor,
};

use super::ScenePart;

pub struct Points2DPart;

impl ScenePart for Points2DPart {
    fn load(
        scene: &mut crate::ui::view_spatial::SceneSpatial,
        ctx: &mut crate::misc::ViewerContext<'_>,
        query: &crate::ui::scene::SceneQuery<'_>,
        transforms: &crate::ui::transform_cache::TransformCache,
        objects_properties: &re_data_store::ObjectsProperties,
        hovered_instance: re_data_store::InstanceIdHash,
    ) {
        crate::profile_function!("load_points2d");

        // Ensure keypoint connection lines are behind points.
        let connection_depth = scene.primitives.line_strips.next_2d_z;
        let point_depth = scene.primitives.line_strips.next_2d_z - 0.1;

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Point2D])
        {
            let mut label_batch = Vec::new();
            let max_num_labels = 10;

            let annotations = scene.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);
            let properties = objects_properties.get(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

            // If keypoints ids show up we may need to connect them later!
            // We include time in the key, so that the "Visible history" (time range queries) feature works.
            let mut keypoints: HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec3>> =
                Default::default();

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
                            .insert(keypoint_id, pos.extend(connection_depth));
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
                    .add_points(
                        [pos.extend(point_depth), pos.extend(point_depth - 0.1)].into_iter(),
                    )
                    .colors([paint_props.bg_stroke.color, paint_props.fg_stroke.color].into_iter())
                    .radii(
                        [
                            Size::new_points(paint_props.bg_stroke.width * 0.5),
                            Size::new_points(paint_props.fg_stroke.width * 0.5),
                        ]
                        .into_iter(),
                    )
                    .user_data_splat(instance_hash);

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
