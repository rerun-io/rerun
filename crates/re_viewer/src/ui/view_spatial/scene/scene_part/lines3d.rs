use glam::{Mat4, Vec3};

use re_arrow_store::LatestAtQuery;
use re_data_store::{query::visit_type_data_2, FieldName, InstanceIdHash, ObjPath, ObjectProps};
use re_log_types::{
    field_types::{ColorRGBA, Instance, LineStrip3D, Radius},
    msg_bundle::Component,
    DataVec, IndexHash, MsgId, ObjectType,
};
use re_query::{query_entity_with_primary, EntityView, QueryError};
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

pub struct Lines3DPartClassic;

impl ScenePart for Lines3DPartClassic {
    /// Both `Path3D` and `LineSegments3D`.

    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_scope!("Lines3DPart");

        for (obj_type, obj_path, time_query, obj_store) in query.iter_object_stores(
            ctx.log_db,
            &[ObjectType::Path3D, ObjectType::LineSegments3D],
        ) {
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

                let mut radius =
                    stroke_width.map_or(Size::NORMAL_LINE, |w| Size::new_scene(w / 2.0));

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

pub struct Lines3DPart;

impl Lines3DPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        scene: &mut SceneSpatial,
        _query: &SceneQuery<'_>,
        props: &ObjectProps,
        hovered_instance: InstanceIdHash,
        entity_view: &EntityView<LineStrip3D>,
        ent_path: &ObjPath,
        world_from_obj: Mat4,
    ) -> Result<(), QueryError> {
        scene.num_logged_3d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::ObjPath(ent_path);

        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("lines 3d")
            .world_from_obj(world_from_obj);

        let visitor = |instance: Instance,
                       strip: LineStrip3D,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>| {
            let instance_hash = {
                if props.interactive {
                    InstanceIdHash::from_path_and_arrow_instance(ent_path, &instance)
                } else {
                    InstanceIdHash::NONE
                }
            };

            let mut radius = radius.map_or(Size::NORMAL_LINE, |r| Size::new_scene(r.0));

            // TODO(andreas): support class ids for lines
            let annotation_info = annotations.class_description(None).annotation_info();
            let mut color = to_ecolor(
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color),
            );

            if instance_hash.is_some() && instance_hash == hovered_instance {
                color = SceneSpatial::HOVER_COLOR;
                radius = SceneSpatial::hover_size_boost(radius);
            }

            line_batch
                .add_strip(strip.0.into_iter().map(|v| v.into()))
                .radius(radius)
                .color(color)
                .user_data(instance_hash);
        };

        entity_view.visit3(visitor)?;

        Ok(())
    }
}

impl ScenePart for Lines3DPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        hovered_instance: re_data_store::InstanceIdHash,
    ) {
        crate::profile_scope!("Lines3DPart");

        for (ent_path, props) in query.iter_entities() {
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(ent_path) else {
                continue;
            };

            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            match query_entity_with_primary::<LineStrip3D>(
                &ctx.log_db.obj_db.arrow_store,
                &timeline_query,
                ent_path,
                &[ColorRGBA::name(), Radius::name()],
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
