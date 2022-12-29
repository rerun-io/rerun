use re_data_store::{query::visit_type_data_2, FieldName, InstanceIdHash, ObjectsProperties};
use re_log_types::{DataVec, IndexHash, MsgId, ObjectType};
use re_renderer::Size;

use crate::{
    misc::ViewerContext,
    ui::{
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::{
            scene::{apply_hover_effect, instance_hash_if_interactive, paint_properties},
            SceneSpatial,
        },
        DefaultColor,
    },
};

use super::ScenePart;

pub struct LineSegments2DPart;

impl ScenePart for LineSegments2DPart {
    fn load(
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        objects_properties: &ObjectsProperties,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        // TODO(andreas): Workaround for unstable z index when interacting on images.
        //                See also https://github.com/rerun-io/rerun/issues/647
        scene.primitives.line_strips.next_2d_z = -0.0001;

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::LineSegments2D])
        {
            let annotations = scene.annotation_map.find(obj_path);
            let properties = objects_properties.get(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

            let mut line_batch = scene
                .primitives
                .line_strips
                .batch("lines 2d")
                .world_from_obj(world_from_obj);

            let visitor = |instance_index: Option<&IndexHash>,
                           _time: i64,
                           _msg_id: &MsgId,
                           points: &DataVec,
                           color: Option<&[u8; 4]>,
                           stroke_width: Option<&f32>| {
                let Some(points) = points.as_vec_of_vec2("LineSegments2D::points")
                                else { return };

                let instance_hash =
                    instance_hash_if_interactive(obj_path, instance_index, properties.interactive);

                // TODO(andreas): support class ids for line segments
                let annotation_info = annotations.class_description(None).annotation_info();
                let color = annotation_info.color(color, DefaultColor::ObjPath(obj_path));

                let mut paint_props = paint_properties(color, stroke_width);
                if instance_hash.is_some() && hovered_instance == instance_hash {
                    apply_hover_effect(&mut paint_props);
                }

                // TODO(andreas): support outlines directly by re_renderer (need only 1 and 2 *point* black outlines)
                line_batch
                    .add_segments_2d(points.chunks_exact(2).map(|chunk| {
                        (
                            glam::vec2(chunk[0][0], chunk[0][1]),
                            glam::vec2(chunk[1][0], chunk[1][1]),
                        )
                    }))
                    .color(paint_props.bg_stroke.color)
                    .radius(Size::new_points(paint_props.bg_stroke.width * 0.5))
                    .user_data(instance_hash);
                line_batch
                    .add_segments_2d(points.chunks_exact(2).map(|chunk| {
                        (
                            glam::vec2(chunk[0][0], chunk[0][1]),
                            glam::vec2(chunk[1][0], chunk[1][1]),
                        )
                    }))
                    .color(paint_props.fg_stroke.color)
                    .radius(Size::new_points(paint_props.fg_stroke.width * 0.5));
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
