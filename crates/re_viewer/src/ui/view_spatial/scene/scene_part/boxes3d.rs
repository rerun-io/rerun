use glam::{vec3, Vec3};

use re_data_store::{query::visit_type_data_4, FieldName, InstanceIdHash, ObjectsProperties};
use re_log_types::{context::ClassId, IndexHash, MsgId, ObjectType};
use re_renderer::Size;

use crate::{
    misc::ViewerContext,
    ui::{
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::{
            scene::{instance_hash_if_interactive, to_ecolor},
            Label3D, SceneSpatial,
        },
        DefaultColor,
    },
};

use super::ScenePart;

pub struct Boxes3DPart;

impl ScenePart for Boxes3DPart {
    fn load(
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        objects_properties: &ObjectsProperties,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Box3D])
        {
            let annotations = scene.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);
            let properties = objects_properties.get(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };
            let mut line_batch = scene
                .primitives
                .line_strips
                .batch("box 3d")
                .world_from_obj(world_from_obj);

            let visitor = |instance_index: Option<&IndexHash>,
                           _time: i64,
                           _msg_id: &MsgId,
                           obb: &re_log_types::Box3,
                           color: Option<&[u8; 4]>,
                           stroke_width: Option<&f32>,
                           label: Option<&String>,
                           class_id: Option<&i32>| {
                let mut line_radius = stroke_width.map_or(Size::AUTO, |w| Size::new_scene(w / 2.0));

                let annotation_info = annotations
                    .class_description(class_id.map(|i| ClassId(*i as _)))
                    .annotation_info();
                let mut color = to_ecolor(annotation_info.color(color, default_color));
                let label = annotation_info.label(label);
                if let Some(label) = label {
                    scene.ui.labels_3d.push(Label3D {
                        text: label,
                        origin: world_from_obj.transform_point3(Vec3::from(obb.translation)),
                    });
                }

                let instance_hash =
                    instance_hash_if_interactive(obj_path, instance_index, properties.interactive);
                if instance_hash.is_some() && instance_hash == hovered_instance {
                    color = SceneSpatial::HOVER_COLOR;
                    line_radius = SceneSpatial::hover_size_boost(line_radius);
                }

                let transform = glam::Affine3A::from_scale_rotation_translation(
                    Vec3::from(obb.half_size),
                    glam::Quat::from_array(obb.rotation),
                    Vec3::from(obb.translation),
                );

                let corners = [
                    transform.transform_point3(vec3(-0.5, -0.5, -0.5)),
                    transform.transform_point3(vec3(-0.5, -0.5, 0.5)),
                    transform.transform_point3(vec3(-0.5, 0.5, -0.5)),
                    transform.transform_point3(vec3(-0.5, 0.5, 0.5)),
                    transform.transform_point3(vec3(0.5, -0.5, -0.5)),
                    transform.transform_point3(vec3(0.5, -0.5, 0.5)),
                    transform.transform_point3(vec3(0.5, 0.5, -0.5)),
                    transform.transform_point3(vec3(0.5, 0.5, 0.5)),
                ];

                line_batch
                    .add_segments(
                        [
                            // bottom:
                            (corners[0b000], corners[0b001]),
                            (corners[0b000], corners[0b010]),
                            (corners[0b011], corners[0b001]),
                            (corners[0b011], corners[0b010]),
                            // top:
                            (corners[0b100], corners[0b101]),
                            (corners[0b100], corners[0b110]),
                            (corners[0b111], corners[0b101]),
                            (corners[0b111], corners[0b110]),
                            // sides:
                            (corners[0b000], corners[0b100]),
                            (corners[0b001], corners[0b101]),
                            (corners[0b010], corners[0b110]),
                            (corners[0b011], corners[0b111]),
                        ]
                        .into_iter(),
                    )
                    .radius(line_radius)
                    .color(color)
                    .user_data(instance_hash);
            };

            visit_type_data_4(
                obj_store,
                &FieldName::from("obb"),
                &time_query,
                ("color", "stroke_width", "label", "class_id"),
                visitor,
            );
        }
    }
}
