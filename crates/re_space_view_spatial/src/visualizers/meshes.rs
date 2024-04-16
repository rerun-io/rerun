use re_entity_db::EntityPath;
use re_log_types::{RowId, TimeInt};
use re_query2::{range_zip_1x6, Results};
use re_renderer::renderer::MeshInstance;
use re_types::{
    archetypes::Mesh3D,
    components::{
        Color, InstanceKey, Material, MeshProperties, Position3D, TensorData, Texcoord2D, Vector3D,
    },
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, SpaceViewSystemExecutionError, ViewContextCollection,
    ViewQuery, ViewerContext, VisualizableEntities, VisualizableFilterContext, VisualizerQueryInfo,
    VisualizerSystem,
};

use crate::{
    contexts::{EntityDepthOffsets, SpatialSceneEntityContext},
    instance_hash_conversions::picking_layer_id_from_instance_path_hash,
    mesh_cache::{AnyMesh, MeshCache, MeshCacheKey},
    view_kind::SpatialSpaceViewKind,
};

use super::{filter_visualizable_3d_entities, SpatialViewVisualizerData};

// ---

pub struct Mesh3DVisualizer(SpatialViewVisualizerData);

impl Default for Mesh3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialSpaceViewKind::ThreeD,
        )))
    }
}

struct Mesh3DComponentData {
    index: (TimeInt, RowId),
    mesh: Mesh3D,
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Mesh3DVisualizer {
    fn process_data(
        &mut self,
        ctx: &ViewerContext<'_>,
        instances: &mut Vec<MeshInstance>,
        entity_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Mesh3DComponentData>,
    ) {
        for data in data {
            let primary_row_id = data.index.1;
            let picking_instance_hash = re_entity_db::InstancePathHash::entity_splat(entity_path);
            let outline_mask_ids = ent_context.highlight.index_outline_mask(InstanceKey::SPLAT);

            let mesh = ctx.cache.entry(|c: &mut MeshCache| {
                let key = MeshCacheKey {
                    versioned_instance_path_hash: picking_instance_hash.versioned(primary_row_id),
                    media_type: None,
                };
                c.entry(
                    &entity_path.to_string(),
                    key.clone(),
                    AnyMesh::Mesh {
                        mesh: &data.mesh,
                        texture_key: re_log_types::hash::Hash64::hash(&key).hash64(),
                    },
                    ctx.render_ctx,
                )
            });

            if let Some(mesh) = mesh {
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

                self.0.add_bounding_box(
                    entity_path.hash(),
                    mesh.bbox(),
                    ent_context.world_from_entity,
                );
            };
        }
    }
}

impl IdentifiedViewSystem for Mesh3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Mesh3D".into()
    }
}

impl VisualizerSystem for Mesh3DVisualizer {
    fn visualizer_query_info(&self) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Mesh3D>()
    }

    fn filter_visualizable_entities(
        &self,
        entities: ApplicableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_3d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewerContext<'_>,
        view_query: &ViewQuery<'_>,
        view_ctx: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, SpaceViewSystemExecutionError> {
        let mut instances = Vec::new();

        // TODO(#5974): this should use the cached APIs, but for that we first need to figure out
        // how to distribute memory budgets across all caching layers, and how to GC them.
        super::entity_iterator::process_archetype_uncached::<Mesh3DVisualizer, Mesh3D, _>(
            ctx,
            view_query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            |ctx, entity_path, _entity_props, spatial_ctx, results| match results {
                Results::LatestAt(_query, results) => {
                    re_tracing::profile_scope!(format!("{entity_path} @ {_query:?}"));

                    use crate::visualizers::LatestAtResultsExt as _;

                    let resolver = ctx.recording().resolver();

                    let vertex_positions = match results.get_dense::<Position3D>(resolver) {
                        Some(Ok(positions)) if !positions.is_empty() => positions,
                        Some(err @ Err(_)) => err?,
                        _ => return Ok(()),
                    };

                    let data = {
                        // NOTE:
                        // - Per-vertex properties are joined using the cluster key as usual.
                        // - Per-mesh properties are just treated as a "global var", essentially.
                        Mesh3DComponentData {
                            index: results.compound_index,
                            mesh: Mesh3D {
                                vertex_positions,
                                vertex_normals: results
                                    .get_dense::<Vector3D>(resolver)
                                    .map_or(Ok(None), |v| v.map(Some))?,
                                vertex_colors: results
                                    .get_dense::<Color>(resolver)
                                    .map_or(Ok(None), |v| v.map(Some))?,
                                vertex_texcoords: results
                                    .get_dense::<Texcoord2D>(resolver)
                                    .map_or(Ok(None), |v| v.map(Some))?,
                                mesh_properties: results
                                    .get_dense::<MeshProperties>(resolver)
                                    .map_or(Ok(None), |v| v.map(|mut v| v.pop()))?,
                                mesh_material: results
                                    .get_dense::<Material>(resolver)
                                    .map_or(Ok(None), |v| v.map(|mut v| v.pop()))?,
                                albedo_texture: results
                                    .get_dense::<TensorData>(resolver)
                                    .map_or(Ok(None), |v| v.map(|mut v| v.pop()))?,
                                class_ids: None,
                            },
                        }
                    };

                    self.process_data(
                        ctx,
                        &mut instances,
                        entity_path,
                        spatial_ctx,
                        std::iter::once(data),
                    );
                    Ok(())
                }

                Results::Range(_query, results) => {
                    re_tracing::profile_scope!(format!("{entity_path} @ {_query:?}"));

                    use crate::visualizers::RangeResultsExt as _;

                    let resolver = ctx.recording().resolver();

                    let Some(vertex_positions) = results.get_dense::<Position3D>(resolver) else {
                        return Ok(());
                    };

                    let vertex_normals = results.get_or_empty_dense(resolver);
                    let vertex_colors = results.get_or_empty_dense(resolver);
                    let vertex_texcoords = results.get_or_empty_dense(resolver);
                    let mesh_properties = results.get_or_empty_dense(resolver);
                    let mesh_materials = results.get_or_empty_dense(resolver);
                    let albedo_textures = results.get_or_empty_dense(resolver);

                    let data = range_zip_1x6(
                        vertex_positions,
                        vertex_normals,
                        vertex_colors,
                        vertex_texcoords,
                        mesh_properties,
                        mesh_materials,
                        albedo_textures,
                    )
                    .map(
                        |(
                            index,
                            vertex_positions,
                            vertex_normals,
                            vertex_colors,
                            vertex_texcoords,
                            mesh_properties,
                            mesh_material,
                            albedo_texture,
                        )| {
                            Mesh3DComponentData {
                                index,
                                mesh: Mesh3D {
                                    vertex_positions,
                                    vertex_normals,
                                    vertex_colors,
                                    vertex_texcoords,
                                    mesh_properties: mesh_properties
                                        .and_then(|mut props| props.pop()),
                                    mesh_material: mesh_material.and_then(|mut mats| mats.pop()),
                                    albedo_texture: albedo_texture.and_then(|mut texs| texs.pop()),
                                    class_ids: None,
                                },
                            }
                        },
                    );

                    self.process_data(ctx, &mut instances, entity_path, spatial_ctx, data);
                    Ok(())
                }
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
