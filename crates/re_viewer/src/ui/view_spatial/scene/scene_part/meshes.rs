use egui::Color32;
use glam::Mat4;

use re_data_store::{query::visit_type_data_1, FieldName, InstanceIdHash, ObjPath, ObjectProps};
use re_log_types::{
    field_types::{ColorRGBA, Instance},
    msg_bundle::Component,
    IndexHash, Mesh3D, MsgId, ObjectType,
};
use re_query::{query_primary_with_history, EntityView, QueryError};

use crate::{
    misc::ViewerContext,
    ui::{
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::{
            scene::instance_hash_if_interactive, MeshSource, MeshSourceData, SceneSpatial,
        },
        DefaultColor,
    },
};

use super::ScenePart;

pub struct MeshPartClassic;

impl ScenePart for MeshPartClassic {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
    ) {
        crate::profile_scope!("MeshPartClassic");

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Mesh3D])
        {
            scene.num_logged_3d_objects += 1;

            let properties = query.obj_props.get(obj_path);
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

            // TODO(andreas): This throws away perspective transformation!
            let world_from_obj_affine = glam::Affine3A::from_mat4(world_from_obj);

            let hovered_paths = ctx.hovered().check_obj_path(obj_path.hash());
            let selected_paths = ctx.selection().check_obj_path(obj_path.hash());

            let visitor = |instance_index: Option<&IndexHash>,
                           _time: i64,
                           _msg_id: &MsgId,
                           mesh: &re_log_types::Mesh3D,
                           _color: Option<&[u8; 4]>| {
                let instance_hash =
                    instance_hash_if_interactive(obj_path, instance_index, properties.interactive);

                let additive_tint = SceneSpatial::apply_hover_and_selection_effect_color(
                    Color32::TRANSPARENT,
                    hovered_paths.contains_index(instance_hash.instance_index_hash),
                    selected_paths.contains_index(instance_hash.instance_index_hash),
                );

                if let Some(mesh) = ctx
                    .cache
                    .mesh
                    .load(
                        &obj_path.to_string(),
                        &MeshSourceData::Mesh3D(mesh.clone()),
                        ctx.render_ctx,
                    )
                    .map(|cpu_mesh| MeshSource {
                        instance_hash,
                        world_from_mesh: world_from_obj_affine,
                        mesh: cpu_mesh,
                        additive_tint,
                    })
                {
                    scene.primitives.meshes.push(mesh);
                };
            };

            visit_type_data_1(
                obj_store,
                &FieldName::from("mesh"),
                &time_query,
                ("color",),
                visitor,
            );
        }
    }
}

pub struct MeshPart;

impl MeshPart {
    #[allow(clippy::too_many_arguments)]
    fn process_entity_view(
        scene: &mut SceneSpatial,
        _query: &SceneQuery<'_>,
        props: &ObjectProps,
        entity_view: &EntityView<Mesh3D>,
        ent_path: &ObjPath,
        world_from_obj: Mat4,
        ctx: &mut ViewerContext<'_>,
    ) -> Result<(), QueryError> {
        scene.num_logged_3d_objects += 1;

        let _default_color = DefaultColor::ObjPath(ent_path);
        let world_from_obj_affine = glam::Affine3A::from_mat4(world_from_obj);

        let hovered_paths = ctx.hovered().check_obj_path(ent_path.hash());
        let selected_paths = ctx.selection().check_obj_path(ent_path.hash());

        let visitor =
            |instance: Instance, mesh: re_log_types::Mesh3D, _color: Option<ColorRGBA>| {
                let instance_hash = {
                    if props.interactive {
                        InstanceIdHash::from_path_and_arrow_instance(ent_path, &instance)
                    } else {
                        InstanceIdHash::NONE
                    }
                };

                let additive_tint = SceneSpatial::apply_hover_and_selection_effect_color(
                    Color32::TRANSPARENT,
                    hovered_paths.contains_index(instance_hash.instance_index_hash),
                    selected_paths.contains_index(instance_hash.instance_index_hash),
                );

                if let Some(mesh) = ctx
                    .cache
                    .mesh
                    .load(
                        &ent_path.to_string(),
                        &MeshSourceData::Mesh3D(mesh),
                        ctx.render_ctx,
                    )
                    .map(|cpu_mesh| MeshSource {
                        instance_hash,
                        world_from_mesh: world_from_obj_affine,
                        mesh: cpu_mesh,
                        additive_tint,
                    })
                {
                    scene.primitives.meshes.push(mesh);
                };
            };

        entity_view.visit2(visitor)?;

        Ok(())
    }
}

impl ScenePart for MeshPart {
    fn load(
        &self,
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
    ) {
        crate::profile_scope!("MeshPart");

        for (ent_path, props) in query.iter_entities() {
            let ReferenceFromObjTransform::Reachable(world_from_obj) = transforms.reference_from_obj(ent_path) else {
                continue;
            };

            match query_primary_with_history::<Mesh3D, 3>(
                &ctx.log_db.obj_db.arrow_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [Mesh3D::name(), Instance::name(), ColorRGBA::name()],
            )
            .and_then(|entities| {
                for entity in entities {
                    Self::process_entity_view(
                        scene,
                        query,
                        &props,
                        &entity,
                        ent_path,
                        world_from_obj,
                        ctx,
                    )?;
                }
                Ok(())
            }) {
                Ok(_) | Err(QueryError::PrimaryNotFound) => {}
                Err(err) => {
                    re_log::error_once!("Unexpected error querying '{:?}': {:?}", ent_path, err);
                }
            }
        }
    }
}
