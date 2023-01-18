use glam::{Mat4, Vec3};

use re_arrow_store::LatestAtQuery;
use re_data_store::{query::visit_type_data_4, FieldName, InstanceIdHash, ObjPath, ObjectProps};
use re_log_types::{
    field_types::{Box3D, ClassId, ColorRGBA, Instance, Label, Quaternion, Radius, Vec3D},
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
            scene::{instance_hash_if_interactive, to_ecolor},
            Label3D, SceneSpatial,
        },
        DefaultColor,
    },
};

use super::ScenePart;

pub struct Boxes3DPartClassic;

impl ScenePart for Boxes3DPartClassic {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
    ) {
        crate::profile_scope!("Boxes3DPartClassic");

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Box3D])
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
                if highlighted_paths.index_part_of_selection(instance_hash.instance_index_hash) {
                    color = SceneSpatial::HOVER_COLOR;
                    line_radius = SceneSpatial::hover_size_boost(line_radius);
                }

                let transform = glam::Affine3A::from_scale_rotation_translation(
                    Vec3::from(obb.half_size),
                    glam::Quat::from_array(obb.rotation),
                    Vec3::from(obb.translation),
                );
                line_batch
                    .add_box_outline(transform)
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

pub struct Boxes3DPart;

impl Boxes3DPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        props: &ObjectProps,
        entity_view: &EntityView<Box3D>,
        ent_path: &ObjPath,
        world_from_obj: Mat4,
    ) -> Result<(), QueryError> {
        scene.num_logged_3d_objects += 1;

        let annotations = scene.annotation_map.find(ent_path);
        let default_color = DefaultColor::ObjPath(ent_path);

        let mut line_batch = scene
            .primitives
            .line_strips
            .batch("box 3d")
            .world_from_obj(world_from_obj);

        let highlighted_paths = ctx.hovered().is_obj_path_selected(ent_path.hash());

        let visitor = |instance: Instance,
                       half_size: Box3D,
                       position: Option<Vec3D>,
                       rotation: Option<Quaternion>,
                       color: Option<ColorRGBA>,
                       radius: Option<Radius>,
                       label: Option<Label>,
                       class_id: Option<ClassId>| {
            let instance_hash = {
                if props.interactive {
                    InstanceIdHash::from_path_and_arrow_instance(ent_path, &instance)
                } else {
                    InstanceIdHash::NONE
                }
            };

            let class_description = annotations.class_description(class_id);
            let annotation_info = class_description.annotation_info();

            let mut radius = radius.map_or(Size::AUTO, |r| Size::new_scene(r.0));
            let mut color = to_ecolor(
                annotation_info.color(color.map(move |c| c.to_array()).as_ref(), default_color),
            );

            if highlighted_paths.index_part_of_selection(instance_hash.instance_index_hash) {
                color = SceneSpatial::HOVER_COLOR;
                radius = SceneSpatial::hover_size_boost(radius);
            }

            let scale = glam::Vec3::from(half_size);
            let rot = rotation.map(glam::Quat::from).unwrap_or_default();
            let tran = position.map_or(glam::Vec3::ZERO, glam::Vec3::from);
            let transform = glam::Affine3A::from_scale_rotation_translation(scale, rot, tran);

            line_batch
                .add_box_outline(transform)
                .radius(radius)
                .color(color)
                .user_data(instance_hash);

            if let Some(label) = annotation_info.label(label.as_ref().map(|s| &s.0)) {
                scene.ui.labels_3d.push(Label3D {
                    text: label,
                    origin: world_from_obj.transform_point3(tran),
                });
            }
        };

        entity_view.visit7(visitor)
    }
}

impl ScenePart for Boxes3DPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
    ) {
        crate::profile_scope!("Boxes3DPart");

        for (ent_path, props) in query.iter_entities() {
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(ent_path) else {
                continue;
            };

            let timeline_query = LatestAtQuery::new(query.timeline, query.latest_at);

            match query_entity_with_primary::<Box3D>(
                &ctx.log_db.obj_db.arrow_store,
                &timeline_query,
                ent_path,
                &[
                    Vec3D::name(),      // obb.position
                    Quaternion::name(), // obb.rotation
                    ColorRGBA::name(),
                    Radius::name(), // stroke_width
                    Label::name(),
                    ClassId::name(),
                ],
            )
            .and_then(|entity_view| {
                Self::process_entity_view(
                    scene,
                    ctx,
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
