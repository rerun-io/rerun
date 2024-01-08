use re_entity_db::EntityPath;
use re_query::{ArchetypeView, QueryError};
use re_renderer::renderer::MeshInstance;
use re_types::{
    archetypes::Mesh3D,
    components::{Color, InstanceKey, Material, MeshProperties, Position3D, Vector3D},
    Archetype, ComponentNameSet,
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewQuery, ViewerContext, VisualizableEntities, VisualizableFilterContext, VisualizerSystem,
};

use super::{
    entity_iterator::process_archetype_views, filter_visualizable_3d_entities,
    SpatialViewVisualizerData,
};
use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    instance_hash_conversions::picking_layer_id_from_instance_path_hash,
    mesh_cache::{AnyMesh, MeshCache, MeshCacheKey},
    view_kind::SpatialSpaceViewKind,
};

pub struct Mesh3DVisualizer(SpatialViewVisualizerData);

impl Default for Mesh3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialSpaceViewKind::ThreeD,
        )))
    }
}

impl Mesh3DVisualizer {
    fn process_arch_view(
        &mut self,
        ctx: &ViewerContext<'_>,
        instances: &mut Vec<MeshInstance>,
        arch_view: &ArchetypeView<Mesh3D>,
        ent_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
    ) -> Result<(), QueryError> {
        re_tracing::profile_function!();

        let vertex_positions: Vec<_> = {
            re_tracing::profile_scope!("vertex_positions");
            arch_view.iter_required_component::<Position3D>()?.collect()
        };
        if vertex_positions.is_empty() {
            return Ok(());
        }

        let mesh = {
            re_tracing::profile_scope!("collect");
            // NOTE:
            // - Per-vertex properties are joined using the cluster key as usual.
            // - Per-mesh properties are just treated as a "global var", essentially.
            Mesh3D {
                vertex_positions,
                vertex_normals: if arch_view.has_component::<Vector3D>() {
                    re_tracing::profile_scope!("vertex_normals");
                    Some(
                        arch_view
                            .iter_optional_component::<Vector3D>()?
                            .map(|comp| comp.unwrap_or(Vector3D::ZERO))
                            .collect(),
                    )
                } else {
                    None
                },
                vertex_colors: if arch_view.has_component::<Color>() {
                    re_tracing::profile_scope!("vertex_colors");
                    let fallback = Color::new(0xFFFFFFFF);
                    Some(
                        arch_view
                            .iter_optional_component::<Color>()?
                            .map(|comp| comp.unwrap_or(fallback))
                            .collect(),
                    )
                } else {
                    None
                },
                mesh_properties: arch_view.raw_optional_mono_component::<MeshProperties>()?,
                mesh_material: arch_view.raw_optional_mono_component::<Material>()?,
                class_ids: None,
                instance_keys: None,
            }
        };

        let primary_row_id = arch_view.primary_row_id();
        let picking_instance_hash = re_entity_db::InstancePathHash::entity_splat(ent_path);
        let outline_mask_ids = ent_context.highlight.index_outline_mask(InstanceKey::SPLAT);

        let mesh = ctx.cache.entry(|c: &mut MeshCache| {
            c.entry(
                &ent_path.to_string(),
                MeshCacheKey {
                    versioned_instance_path_hash: picking_instance_hash.versioned(primary_row_id),
                    media_type: None,
                },
                AnyMesh::Mesh(&mesh),
                ctx.render_ctx,
            )
        });

        if let Some(mesh) = mesh {
            re_tracing::profile_scope!("mesh instances");

            instances.extend(mesh.mesh_instances.iter().map(move |mesh_instance| {
                let entity_from_mesh = mesh_instance.world_from_mesh;
                let world_from_mesh = ent_context.world_from_entity * entity_from_mesh;

                MeshInstance {
                    gpu_mesh: mesh_instance.gpu_mesh.clone(),
                    world_from_mesh,
                    outline_mask_ids,
                    picking_layer_id: picking_layer_id_from_instance_path_hash(
                        picking_instance_hash,
                    ),
                    ..Default::default()
                }
            }));

            self.0
                .extend_bounding_box(mesh.bbox(), ent_context.world_from_entity);
        };

        Ok(())
    }
}

impl IdentifiedViewSystem for Mesh3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Mesh3D".into()
    }
}

impl VisualizerSystem for Mesh3DVisualizer {
    fn required_components(&self) -> ComponentNameSet {
        Mesh3D::required_components()
            .iter()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn indicator_components(&self) -> ComponentNameSet {
        std::iter::once(Mesh3D::indicator().name()).collect()
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        filter_visualizable_3d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let mut instances = Vec::new();

        process_archetype_views::<Mesh3DVisualizer, Mesh3D, { Mesh3D::NUM_COMPONENTS }, _>(
            ctx,
            query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            |ctx, ent_path, _ent_props, arch_view, ent_context| {
                self.process_arch_view(ctx, &mut instances, &arch_view, ent_path, ent_context)
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
