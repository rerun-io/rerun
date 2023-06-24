use re_components::{ColorRGBA, Component, InstanceKey, Mesh3D};
use re_data_store::EntityPath;
use re_query::{EntityView, QueryError};
use re_renderer::renderer::MeshInstance;
use re_viewer_context::{
    ArchetypeDefinition, DefaultColor, ScenePart, SceneQuery, SpaceViewHighlights, ViewerContext,
};

use crate::instance_hash_conversions::picking_layer_id_from_instance_path_hash;
use crate::mesh_cache::MeshCache;
use crate::scene::{
    contexts::{SpatialSceneContext, SpatialSceneEntityContext},
    parts::entity_iterator::process_entity_views,
};
use crate::SpatialSpaceView;

use super::{SpatialScenePartData, SpatialSpaceViewState};

#[derive(Default)]
pub struct MeshPart(SpatialScenePartData);

impl MeshPart {
    fn process_entity_view(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        instances: &mut Vec<MeshInstance>,
        ent_view: &EntityView<Mesh3D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        let _default_color = DefaultColor::EntityPath(ent_path);

        let visitor = |instance_key: InstanceKey,
                       mesh: re_components::Mesh3D,
                       _color: Option<ColorRGBA>| {
            let picking_instance_hash =
                re_data_store::InstancePathHash::instance(ent_path, instance_key);

            let outline_mask_ids = ent_context.highlight.index_outline_mask(instance_key);

            let mesh = ctx
                .cache
                .entry(|c: &mut MeshCache| c.entry(&ent_path.to_string(), &mesh, ctx.render_ctx));
            if let Some(mesh) = mesh {
                instances.extend(mesh.mesh_instances.iter().map(move |mesh_instance| {
                    MeshInstance {
                        gpu_mesh: mesh_instance.gpu_mesh.clone(),
                        world_from_mesh: ent_context.world_from_obj * mesh_instance.world_from_mesh,
                        outline_mask_ids,
                        picking_layer_id: picking_layer_id_from_instance_path_hash(
                            picking_instance_hash,
                        ),
                        ..Default::default()
                    }
                }));

                self.0
                    .extend_bounding_box(*mesh.bbox(), ent_context.world_from_obj);
            };
        };

        ent_view.visit2(visitor)?;

        Ok(())
    }
}

impl ScenePart<SpatialSpaceView> for MeshPart {
    fn archetype(&self) -> ArchetypeDefinition {
        vec1::vec1![Mesh3D::name(), InstanceKey::name(), ColorRGBA::name()]
    }

    fn populate(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        _space_view_state: &SpatialSpaceViewState,
        scene_context: &SpatialSceneContext,
        highlights: &SpaceViewHighlights,
    ) -> Vec<re_renderer::QueueableDrawData> {
        re_tracing::profile_scope!("MeshPart");

        let mut instances = Vec::new();

        process_entity_views::<_, 3, _>(
            ctx,
            query,
            scene_context,
            highlights,
            scene_context.depth_offsets.points,
            self.archetype(),
            |ctx, ent_path, entity_view, ent_context| {
                scene_context
                    .num_3d_primitives
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                self.process_entity_view(ctx, &mut instances, &entity_view, ent_path, ent_context)
            },
        );

        match re_renderer::renderer::MeshDrawData::new(ctx.render_ctx, &instances) {
            Ok(draw_data) => {
                vec![draw_data.into()]
            }
            Err(err) => {
                re_log::error_once!("Failed to create mesh draw data from mesh instances: {err}");
                Vec::new()
            }
        }
    }

    fn data(&self) -> Option<&SpatialScenePartData> {
        Some(&self.0)
    }
}
