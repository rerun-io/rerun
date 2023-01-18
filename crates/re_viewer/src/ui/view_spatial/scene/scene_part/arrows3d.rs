use glam::Mat4;
use re_arrow_store::LatestAtQuery;
use re_data_store::{query::visit_type_data_3, FieldName, InstanceIdHash, ObjPath, ObjectProps};
use re_log_types::{
    field_types::{ColorRGBA, Instance, Label, Radius},
    msg_bundle::Component,
    Arrow3D, IndexHash, MsgId, ObjectType,
};
use re_query::{query_entity_with_primary, EntityView, QueryError};
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

pub struct Arrows3DPartClassic;

impl ScenePart for Arrows3DPartClassic {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
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
            let highlighted_paths = ctx.hovered().is_obj_path_selected(obj_path.hash());

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
                let vector = glam::Vec3::from(*vector);
                let origin = glam::Vec3::from(*origin);

                let mut radius = Size::new_scene(width_scale * 0.5);
                let tip_length = LineStripFlags::get_triangle_cap_tip_length(radius.0);
                let vector_len = vector.length();
                let end = origin + vector * ((vector_len - tip_length) / vector_len);

                let mut color = to_ecolor(color);
                if highlighted_paths.index_part_of_selection(instance_hash.instance_index_hash) {
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

pub struct Arrows3DPart;

impl Arrows3DPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        _query: &SceneQuery<'_>,
        props: &ObjectProps,
        entity_view: &EntityView<Arrow3D>,
        ent_path: &ObjPath,
        world_from_obj: Mat4,
    ) -> Result<(), QueryError> {
        scene.num_logged_3d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::ObjPath(ent_path);

        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("arrows")
            .world_from_obj(world_from_obj);

        let highlighted_paths = ctx.hovered().is_obj_path_selected(ent_path.hash());

        let visitor = |instance: Instance,
                       arrow: Arrow3D,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>,
                       _label: Option<Label>| {
            let instance_hash = {
                if props.interactive {
                    InstanceIdHash::from_path_and_arrow_instance(ent_path, &instance)
                } else {
                    InstanceIdHash::NONE
                }
            };

            // TODO(andreas): support labels
            // TODO(andreas): support class ids for arrows
            let annotation_info = annotations.class_description(None).annotation_info();
            let color =
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color);
            //let label = annotation_info.label(label);

            let re_log_types::Arrow3D { origin, vector } = arrow;

            let vector = glam::Vec3::from(vector);
            let origin = glam::Vec3::from(origin);

            let mut radius = radius.map_or(Size(0.5), |r| Size(r.0));
            let tip_length = LineStripFlags::get_triangle_cap_tip_length(radius.0);
            let vector_len = vector.length();
            let end = origin + vector * ((vector_len - tip_length) / vector_len);

            let mut color = to_ecolor(color);
            if highlighted_paths.index_part_of_selection(instance_hash.instance_index_hash) {
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

        entity_view.visit4(visitor)?;

        Ok(())
    }
}

impl ScenePart for Arrows3DPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
    ) {
        crate::profile_scope!("Points2DPart");

        for (ent_path, props) in query.iter_entities() {
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(ent_path) else {
                continue;
            };

            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            match query_entity_with_primary::<Arrow3D>(
                &ctx.log_db.obj_db.arrow_store,
                &timeline_query,
                ent_path,
                &[ColorRGBA::name(), Radius::name(), Label::name()],
            )
            .and_then(|entity_view| {
                Self::process_entity_view(
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
