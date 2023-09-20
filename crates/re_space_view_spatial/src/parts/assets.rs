use re_components::Mesh3D;
use re_data_store::EntityPath;
use re_query::{EntityView, QueryError};
use re_renderer::renderer::MeshInstance;
use re_types::{
    components::{Color, InstanceKey},
    ComponentNameSet, Loggable as _,
};
use re_viewer_context::{
    NamedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection, ViewPartSystem,
    ViewQuery, ViewerContext,
};

use super::SpatialViewPartData;
use crate::{
    contexts::SpatialSceneEntityContext,
    instance_hash_conversions::picking_layer_id_from_instance_path_hash,
    mesh_cache::{AnyMesh, MeshCache},
    parts::entity_iterator::process_entity_views,
    view_kind::SpatialSpaceViewKind,
};

pub struct Asset3DPart(SpatialViewPartData);

impl Default for Asset3DPart {
    fn default() -> Self {
        Self(SpatialViewPartData::new(Some(SpatialSpaceViewKind::ThreeD)))
    }
}

impl Asset3DPart {
    fn process_entity_view(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        instances: &mut Vec<MeshInstance>,
        ent_view: &EntityView<Mesh3D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        let primary_row_id = ent_view.primary_row_id();
        let visitor = |instance_key: InstanceKey,
                       mesh: re_components::Mesh3D,
                       _color: Option<Color>| {
            let picking_instance_hash =
                re_data_store::InstancePathHash::instance(ent_path, instance_key);

            let outline_mask_ids = ent_context.highlight.index_outline_mask(instance_key);

            let mesh = ctx.cache.entry(|c: &mut MeshCache| {
                c.entry(
                    &ent_path.to_string(),
                    picking_instance_hash.versioned(primary_row_id),
                    AnyMesh::Asset(&mesh),
                    ctx.render_ctx,
                )
            });
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

impl NamedViewSystem for Asset3DPart {
    fn name() -> re_viewer_context::ViewSystemName {
        "Asset3D".into()
    }
}

impl ViewPartSystem for Asset3DPart {
    fn required_components(&self) -> ComponentNameSet {
        std::iter::once(Mesh3D::name()).collect()
    }

    // TODO(#2788): use this instead
    // fn archetype(&self) -> Vec<ComponentName> {
    //     Mesh3D::required_components().to_vec()
    // }

    // TODO(#2788): use this instead
    // fn heuristic_filter(
    //     &self,
    //     _store: &re_arrow_store::DataStore,
    //     _ent_path: &EntityPath,
    //     components: &[re_types::ComponentName],
    // ) -> bool {
    //     components.contains(&Mesh3D::indicator_component())
    // }

    fn execute(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let mut instances = Vec::new();

        let components = [Mesh3D::name(), InstanceKey::name(), Color::name()];
        process_entity_views::<Asset3DPart, _, 3, _>(
            ctx,
            query,
            view_ctx,
            0,
            components.into_iter().collect(),
            |ctx, ent_path, entity_view, ent_context| {
                self.process_entity_view(ctx, &mut instances, &entity_view, ent_path, ent_context)
            },
        )?;

        match re_renderer::renderer::MeshDrawData::new(ctx.render_ctx, &instances) {
            Ok(draw_data) => Ok(vec![draw_data.into()]),
            Err(err) => {
                re_log::error_once!("Failed to create mesh draw data from mesh instances: {err}");
                Ok(Vec::new()) // TODO(andreas): Pass error on?
            }
        }
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
