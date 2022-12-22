use re_data_store::{query::visit_type_data_1, FieldName, InstanceIdHash, ObjectsProperties};
use re_log_types::{IndexHash, MsgId, ObjectType};

use crate::{
    misc::ViewerContext,
    ui::{
        scene::SceneQuery,
        transform_cache::{ReferenceFromObjTransform, TransformCache},
        view_spatial::{
            scene::instance_hash_if_interactive, MeshSource, MeshSourceData, SceneSpatial,
        },
    },
};

use super::ScenePart;

pub struct MeshPart;

impl ScenePart for MeshPart {
    fn load(
        scene: &mut SceneSpatial,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        objects_properties: &ObjectsProperties,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!("load_meshes");

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Mesh3D])
        {
            let properties = objects_properties.get(obj_path);
            let ReferenceFromObjTransform::Rigid(world_from_obj) = transforms.reference_from_obj(obj_path) else {
                continue;
            };

            // TODO(andreas): This throws away perspective transformation!
            let world_from_obj_affine = glam::Affine3A::from_mat4(world_from_obj);

            let visitor = |instance_index: Option<&IndexHash>,
                           _time: i64,
                           _msg_id: &MsgId,
                           mesh: &re_log_types::Mesh3D,
                           _color: Option<&[u8; 4]>| {
                let instance_hash =
                    instance_hash_if_interactive(obj_path, instance_index, properties.interactive);

                let additive_tint = if instance_hash.is_some() && hovered_instance == instance_hash
                {
                    Some(SceneSpatial::HOVER_COLOR)
                } else {
                    None
                };

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
