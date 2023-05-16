use re_data_store::EntityPath;
use re_log_types::{
    component_types::{ColorRGBA, InstanceKey},
    Component, Mesh3D,
};
use re_query::{query_primary_with_history, EntityView, QueryError};
use re_viewer_context::{DefaultColor, SceneQuery, ViewerContext};

use crate::{
    misc::{MeshCache, SpaceViewHighlights, TransformCache},
    ui::view_spatial::{scene::EntityDepthOffsets, MeshSource, SceneSpatial},
};

use super::{instance_path_hash_for_picking, ScenePart};

pub struct MeshPart;

impl MeshPart {
    fn process_entity_view(
        scene: &mut SceneSpatial,
        entity_view: &EntityView<Mesh3D>,
        ent_path: &EntityPath,
        world_from_obj: glam::Affine3A,
        ctx: &mut ViewerContext<'_>,
        highlights: &SpaceViewHighlights,
    ) -> Result<(), QueryError> {
        scene.num_logged_3d_objects += 1;

        let _default_color = DefaultColor::EntityPath(ent_path);
        let entity_highlight = highlights.entity_outline_mask(ent_path.hash());

        let visitor =
            |instance_key: InstanceKey, mesh: re_log_types::Mesh3D, _color: Option<ColorRGBA>| {
                let picking_instance_hash = instance_path_hash_for_picking(
                    ent_path,
                    instance_key,
                    entity_view.num_instances(),
                    entity_highlight.any_selection_highlight,
                );

                let outline_mask_ids = entity_highlight.index_outline_mask(instance_key);

                if let Some(mesh) = ctx
                    .cache
                    .entry::<MeshCache>()
                    .entry(&ent_path.to_string(), &mesh, ctx.render_ctx)
                    .map(|cpu_mesh| MeshSource {
                        picking_instance_hash,
                        world_from_mesh: world_from_obj,
                        mesh: cpu_mesh,
                        outline_mask_ids,
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
        _depth_offsets: &EntityDepthOffsets,
    ) {
        crate::profile_scope!("MeshPart");

        for (ent_path, props) in query.iter_entities() {
            let Some(world_from_obj) = transforms.reference_from_entity(ent_path) else {
                continue;
            };

            match query_primary_with_history::<Mesh3D, 3>(
                &ctx.log_db.entity_db.data_store,
                &query.timeline,
                &query.latest_at,
                &props.visible_history,
                ent_path,
                [Mesh3D::name(), InstanceKey::name(), ColorRGBA::name()],
            )
            .and_then(|entities| {
                for entity in entities {
                    Self::process_entity_view(
                        scene,
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
                    re_log::error_once!("Unexpected error querying {ent_path:?}: {err}");
                }
            }
        }
    }
}
