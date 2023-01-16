use re_data_store::{query::visit_type_data_3, FieldName, InstanceIdHash};
use re_log_types::{IndexHash, MsgId, ObjectType};
use re_renderer::{renderer::LineStripFlags, Size};

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

pub struct Arrows3DPart;

impl ScenePart for Arrows3DPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_scope!("Arrows3DPart");

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Arrow3D])
        {
            scene.num_logged_3d_objects += 1;

            let annotations = scene.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);
            let properties = query.obj_props.get(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

            let mut line_batch = scene
                .primitives
                .line_strips
                .batch("arrows")
                .world_from_obj(world_from_obj);

            let visitor = |instance_index: Option<&IndexHash>,
                           _time: i64,
                           _msg_id: &MsgId,
                           arrow: &re_log_types::Arrow3D,
                           color: Option<&[u8; 4]>,
                           width_scale: Option<&f32>,
                           _label: Option<&String>| {
                // TODO(andreas): support labels
                let instance_hash =
                    instance_hash_if_interactive(obj_path, instance_index, properties.interactive);

                let width = width_scale.copied().unwrap_or(1.0);

                // TODO(andreas): support class ids for arrows
                let annotation_info = annotations.class_description(None).annotation_info();
                let color = annotation_info.color(color, default_color);
                //let label = annotation_info.label(label);

                let width_scale = Some(width);

                let re_log_types::Arrow3D { origin, vector } = arrow;

                let width_scale = width_scale.unwrap_or(1.0);
                let vector: glam::Vec3 = vector.clone().into();
                let origin: glam::Vec3 = origin.clone().into();

                let mut radius = Size::new_scene(width_scale * 0.5);
                let tip_length = LineStripFlags::get_triangle_cap_tip_length(radius.0);
                let vector_len = vector.length();
                let end = origin + vector * ((vector_len - tip_length) / vector_len);

                let mut color = to_ecolor(color);
                if instance_hash.is_some() && instance_hash == hovered_instance {
                    color = SceneSpatial::HOVER_COLOR;
                    radius = SceneSpatial::hover_size_boost(radius);
                }

                line_batch
                    .add_segment(origin, end)
                    .radius(radius)
                    .color(color)
                    .flags(re_renderer::renderer::LineStripFlags::CAP_END_TRIANGLE)
                    .user_data(instance_hash);
            };
            visit_type_data_3(
                obj_store,
                &FieldName::from("arrow3d"),
                &time_query,
                ("color", "width_scale", "label"),
                visitor,
            );
        }
    }
}
