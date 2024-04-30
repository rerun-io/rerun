use itertools::Itertools as _;
use re_entity_db::EntityPath;
use re_log_types::{Instance, RowId, TimeInt};
use re_query::range_zip_1x7;
use re_renderer::renderer::MeshInstance;
use re_types::{
    archetypes::Mesh3D,
    components::{
        ClassId, Color, Material, Position3D, TensorData, Texcoord2D, TriangleIndices, Vector3D,
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

use super::{entity_iterator::clamped, filter_visualizable_3d_entities, SpatialViewVisualizerData};

// ---

pub struct Mesh3DVisualizer(SpatialViewVisualizerData);

impl Default for Mesh3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialSpaceViewKind::ThreeD,
        )))
    }
}

struct Mesh3DComponentData<'a> {
    index: (TimeInt, RowId),

    vertex_positions: &'a [Position3D],
    vertex_normals: &'a [Vector3D],
    vertex_colors: &'a [Color],
    vertex_texcoords: &'a [Texcoord2D],

    triangle_indices: Option<&'a [TriangleIndices]>,
    mesh_material: Option<&'a Material>,
    albedo_texture: Option<&'a TensorData>,

    class_ids: &'a [ClassId],
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Mesh3DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &ViewerContext<'_>,
        instances: &mut Vec<MeshInstance>,
        entity_path: &EntityPath,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Mesh3DComponentData<'a>>,
    ) {
        for data in data {
            let primary_row_id = data.index.1;
            let picking_instance_hash = re_entity_db::InstancePathHash::entity_all(entity_path);
            let outline_mask_ids = ent_context.highlight.index_outline_mask(Instance::ALL);

            let mesh = ctx.cache.entry(|c: &mut MeshCache| {
                let key = MeshCacheKey {
                    versioned_instance_path_hash: picking_instance_hash.versioned(primary_row_id),
                    media_type: None,
                };

                let vertex_normals = clamped(data.vertex_normals, data.vertex_positions.len())
                    .copied()
                    .collect_vec();
                let vertex_colors = clamped(data.vertex_colors, data.vertex_positions.len())
                    .copied()
                    .collect_vec();
                let vertex_texcoords = clamped(data.vertex_texcoords, data.vertex_positions.len())
                    .copied()
                    .collect_vec();

                c.entry(
                    &entity_path.to_string(),
                    key.clone(),
                    AnyMesh::Mesh {
                        mesh: &Mesh3D {
                            vertex_positions: data.vertex_positions.to_owned(),
                            triangle_indices: data.triangle_indices.map(ToOwned::to_owned),
                            vertex_normals: (!vertex_normals.is_empty()).then_some(vertex_normals),
                            vertex_colors: (!vertex_colors.is_empty()).then_some(vertex_colors),
                            vertex_texcoords: (!vertex_texcoords.is_empty())
                                .then_some(vertex_texcoords),
                            mesh_material: data.mesh_material.cloned(),
                            albedo_texture: data.albedo_texture.cloned(),
                            class_ids: (!data.class_ids.is_empty())
                                .then(|| data.class_ids.to_owned()),
                        },
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

        super::entity_iterator::process_archetype::<Mesh3DVisualizer, Mesh3D, _>(
            ctx,
            view_query,
            view_ctx,
            view_ctx.get::<EntityDepthOffsets>()?.points,
            |ctx, entity_path, _entity_props, spatial_ctx, results| {
                re_tracing::profile_scope!(format!("{entity_path}"));

                use crate::visualizers::RangeResultsExt as _;

                let resolver = ctx.recording().resolver();

                let vertex_positions = match results.get_dense::<Position3D>(resolver) {
                    Some(positions) => positions?,
                    _ => return Ok(()),
                };

                let vertex_normals = results.get_or_empty_dense(resolver)?;
                let vertex_colors = results.get_or_empty_dense(resolver)?;
                let vertex_texcoords = results.get_or_empty_dense(resolver)?;
                let triangle_indices = results.get_or_empty_dense(resolver)?;
                let mesh_materials = results.get_or_empty_dense(resolver)?;
                let albedo_textures = results.get_or_empty_dense(resolver)?;
                let class_ids = results.get_or_empty_dense(resolver)?;

                let data = range_zip_1x7(
                    vertex_positions.range_indexed(),
                    vertex_normals.range_indexed(),
                    vertex_colors.range_indexed(),
                    vertex_texcoords.range_indexed(),
                    triangle_indices.range_indexed(),
                    mesh_materials.range_indexed(),
                    albedo_textures.range_indexed(),
                    class_ids.range_indexed(),
                )
                .map(
                    |(
                        &index,
                        vertex_positions,
                        vertex_normals,
                        vertex_colors,
                        vertex_texcoords,
                        triangle_indices,
                        mesh_material,
                        albedo_texture,
                        class_ids,
                    )| {
                        Mesh3DComponentData {
                            index,
                            vertex_positions,
                            vertex_normals: vertex_normals.unwrap_or_default(),
                            vertex_colors: vertex_colors.unwrap_or_default(),
                            vertex_texcoords: vertex_texcoords.unwrap_or_default(),
                            triangle_indices,
                            mesh_material: mesh_material.and_then(|v| v.first()),
                            albedo_texture: albedo_texture.and_then(|v| v.first()),
                            class_ids: class_ids.unwrap_or_default(),
                        }
                    },
                );

                self.process_data(ctx, &mut instances, entity_path, spatial_ctx, data);
                Ok(())
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
