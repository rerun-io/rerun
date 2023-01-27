use egui::Color32;
use glam::Mat4;

use re_data_store::{ObjPath, ObjectProps};
use re_log_types::{
    field_types::{ColorRGBA, Instance},
    msg_bundle::Component,
    Mesh3D,
};
use re_query::{query_primary_with_history, EntityView, QueryError};

use crate::{
    misc::{SpaceViewHighlights, TransformCache, ViewerContext},
    ui::{
        scene::SceneQuery,
        view_spatial::{MeshSource, MeshSourceData, SceneSpatial},
        DefaultColor,
    },
};

use super::{instance_hash_for_picking, ScenePart};

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
        highlights: &SpaceViewHighlights,
    ) -> Result<(), QueryError> {
        scene.num_logged_3d_objects += 1;

        let _default_color = DefaultColor::ObjPath(ent_path);
        let world_from_obj_affine = glam::Affine3A::from_mat4(world_from_obj);
        let object_highlight = highlights.object_highlight(ent_path.hash());

        let visitor = |instance: Instance,
                       mesh: re_log_types::Mesh3D,
                       _color: Option<ColorRGBA>| {
            let instance_hash =
                instance_hash_for_picking(ent_path, instance, entity_view, props, object_highlight);

            let additive_tint = SceneSpatial::apply_hover_and_selection_effect_color(
                Color32::TRANSPARENT,
                object_highlight.index_highlight(instance_hash.instance_index_hash),
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
        highlights: &SpaceViewHighlights,
    ) {
        crate::profile_scope!("MeshPart");

        for (ent_path, props) in query.iter_entities() {
            let Some(world_from_obj) = transforms.reference_from_obj(ent_path) else {
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
                        highlights,
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
