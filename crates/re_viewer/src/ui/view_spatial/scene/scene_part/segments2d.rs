use re_data_store::{query::visit_type_data_2, FieldName, InstanceIdHash};
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

pub struct LineSegments2DPartClassic;

impl ScenePart for LineSegments2DPartClassic {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
    ) {
        crate::profile_scope!("LineSegments2DPart");

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::LineSegments2D])
        {
            scene.num_logged_2d_objects += 1;

            let annotations = scene.annotation_map.find(obj_path);
            let properties = query.obj_props.get(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

            let mut line_batch = scene
                .primitives
                .line_strips
                .batch("lines 2d")
                .world_from_obj(world_from_obj);

            let highlighted_paths = ctx.hovered().is_path_selected(obj_path.hash());

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
                if highlighted_paths.is_index_in_selection(instance_hash.instance_index_hash) {
                    apply_hover_effect(&mut paint_props);
                }

                line_batch
                    .add_segments_2d(points.chunks_exact(2).map(|chunk| {
                        (
                            glam::vec2(chunk[0][0], chunk[0][1]),
                            glam::vec2(chunk[1][0], chunk[1][1]),
                        )
                    }))
                    .color(paint_props.fg_stroke.color)
                    .radius(Size::new_points(paint_props.fg_stroke.width * 0.5))
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
