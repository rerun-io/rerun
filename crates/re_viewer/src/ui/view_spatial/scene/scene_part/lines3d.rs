use glam::Vec3;

use re_data_store::{query::visit_type_data_2, FieldName, InstanceIdHash, ObjectsProperties};
use re_log_types::{DataVec, IndexHash, MsgId, ObjectType};
use re_renderer::Size;

use crate::{
    misc::ViewerContext,
    ui::{
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::{
            scene::{instance_hash_if_interactive, to_ecolor},
            SceneSpatial,
        },
        DefaultColor,
    },
};

use super::ScenePart;

pub struct Lines3DPart;

impl ScenePart for Lines3DPart {
    /// Both `Path3D` and `LineSegments3D`.
    fn load(
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        objects_properties: &ObjectsProperties,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        for (obj_type, obj_path, time_query, obj_store) in query.iter_object_stores(
            ctx.log_db,
            &[ObjectType::Path3D, ObjectType::LineSegments3D],
        ) {
            let annotations = scene.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);
            let properties = objects_properties.get(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

            let mut line_batch = scene
                .primitives
                .line_strips
                .batch("lines 3d")
                .world_from_obj(world_from_obj);

            let visitor = |instance_index: Option<&IndexHash>,
                           _time: i64,
                           _msg_id: &MsgId,
                           points: &DataVec,
                           color: Option<&[u8; 4]>,
                           stroke_width: Option<&f32>| {
                let what = match obj_type {
                    ObjectType::Path3D => "Path3D::points",
                    ObjectType::LineSegments3D => "LineSegments3D::points",
                    _ => return,
                };
                let Some(points) = points.as_vec_of_vec3(what) else { return };
                let instance_hash =
                    instance_hash_if_interactive(obj_path, instance_index, properties.interactive);

                let mut radius = stroke_width.map_or(Size::AUTO, |w| Size::new_scene(w / 2.0));

                // TODO(andreas): support class ids for lines
                let annotation_info = annotations.class_description(None).annotation_info();
                let mut color = to_ecolor(annotation_info.color(color, default_color));

                if instance_hash.is_some() && instance_hash == hovered_instance {
                    color = SceneSpatial::HOVER_COLOR;
                    radius = SceneSpatial::hover_size_boost(radius);
                }

                // Add renderer primitive
                match obj_type {
                    ObjectType::Path3D => {
                        line_batch.add_strip(points.iter().map(|v| Vec3::from_slice(v)))
                    }
                    ObjectType::LineSegments3D => line_batch.add_segments(
                        points
                            .chunks_exact(2)
                            .map(|points| (points[0].into(), points[1].into())),
                    ),
                    _ => unreachable!("already early outed earlier"),
                }
                .radius(radius)
                .color(color)
                .user_data(instance_hash);
            };

            visit_type_data_2(
                obj_store,
                &FieldName::from("points"),
                &time_query,
                ("color", "stroke_width"),
                visitor,
            );
        }
    }
}
