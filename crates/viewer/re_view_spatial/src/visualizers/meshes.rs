use re_chunk_store::RowId;
use re_log_types::{Instance, TimeInt, hash::Hash64};
use re_renderer::{RenderContext, renderer::GpuMeshInstance};
use re_types::{Archetype as _, archetypes::Mesh3D, components::ImageFormat};
use re_viewer_context::{
    IdentifiedViewSystem, MaybeVisualizableEntities, QueryContext, ViewContext,
    ViewContextCollection, ViewQuery, ViewSystemExecutionError, VisualizableEntities,
    VisualizableFilterContext, VisualizerQueryInfo, VisualizerSystem,
};

use super::{SpatialViewVisualizerData, filter_visualizable_3d_entities};

use crate::{
    contexts::SpatialSceneEntityContext,
    mesh_cache::{AnyMesh, MeshCache, MeshCacheKey},
    mesh_loader::NativeMesh3D,
    view_kind::SpatialViewKind,
};

// ---

pub struct Mesh3DVisualizer(SpatialViewVisualizerData);

impl Default for Mesh3DVisualizer {
    fn default() -> Self {
        Self(SpatialViewVisualizerData::new(Some(
            SpatialViewKind::ThreeD,
        )))
    }
}

struct Mesh3DComponentData<'a> {
    index: (TimeInt, RowId),
    query_result_hash: Hash64,
    native_mesh: NativeMesh3D<'a>,
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Mesh3DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        render_ctx: &RenderContext,
        instances: &mut Vec<GpuMeshInstance>,
        ent_context: &SpatialSceneEntityContext<'_>,
        data: impl Iterator<Item = Mesh3DComponentData<'a>>,
    ) {
        let entity_path = ctx.target_entity_path;

        for data in data {
            let primary_row_id = data.index.1;
            let picking_instance_hash = re_entity_db::InstancePathHash::entity_all(entity_path);
            let outline_mask_ids = ent_context.highlight.index_outline_mask(Instance::ALL);

            // Skip over empty meshes.
            // Note that we can deal with zero normals/colors/texcoords/indices just fine (we generate them),
            // but re_renderer insists on having at a non-zero vertex list.
            if data.native_mesh.vertex_positions.is_empty() {
                continue;
            }

            let mesh = ctx.store_ctx().caches.entry(|c: &mut MeshCache| {
                let key = MeshCacheKey {
                    versioned_instance_path_hash: picking_instance_hash.versioned(primary_row_id),
                    query_result_hash: data.query_result_hash,
                    media_type: None,
                };

                c.entry(
                    &entity_path.to_string(),
                    key.clone(),
                    AnyMesh::Mesh {
                        mesh: data.native_mesh,
                        texture_key: re_log_types::hash::Hash64::hash(&key).hash64(),
                    },
                    render_ctx,
                )
            });

            if let Some(mesh) = mesh {
                // Let's draw the mesh once for every instance transform.
                // TODO(#7026): We should formalize this kind of hybrid joining better.
                for &world_from_instance in ent_context
                    .transform_info
                    .reference_from_instances(Mesh3D::name())
                {
                    instances.extend(mesh.mesh_instances.iter().map(move |mesh_instance| {
                        let entity_from_mesh = mesh_instance.world_from_mesh;
                        let world_from_mesh = world_from_instance * entity_from_mesh;

                        GpuMeshInstance {
                            gpu_mesh: mesh_instance.gpu_mesh.clone(),
                            world_from_mesh,
                            outline_mask_ids,
                            picking_layer_id: re_view::picking_layer_id_from_instance_path_hash(
                                picking_instance_hash,
                            ),
                            additive_tint: re_renderer::Color32::TRANSPARENT,
                        }
                    }));

                    self.0
                        .add_bounding_box(entity_path.hash(), mesh.bbox(), world_from_instance);
                }
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
        entities: MaybeVisualizableEntities,
        context: &dyn VisualizableFilterContext,
    ) -> VisualizableEntities {
        re_tracing::profile_function!();
        filter_visualizable_3d_entities(entities, context)
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<Vec<re_renderer::QueueableDrawData>, ViewSystemExecutionError> {
        let mut instances = Vec::new();

        use super::entity_iterator::{iter_slices, process_archetype};
        process_archetype::<Self, Mesh3D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let Some(all_vertex_position_chunks) =
                    results.get_required_chunks(Mesh3D::descriptor_vertex_positions())
                else {
                    return Ok(());
                };

                let timeline = ctx.query.timeline();
                let all_vertex_positions_indexed =
                    iter_slices::<[f32; 3]>(&all_vertex_position_chunks, timeline);
                let all_vertex_normals =
                    results.iter_as(timeline, Mesh3D::descriptor_vertex_normals());
                let all_vertex_colors =
                    results.iter_as(timeline, Mesh3D::descriptor_vertex_colors());
                let all_vertex_texcoords =
                    results.iter_as(timeline, Mesh3D::descriptor_vertex_texcoords());
                let all_triangle_indices =
                    results.iter_as(timeline, Mesh3D::descriptor_triangle_indices());
                let all_albedo_factors =
                    results.iter_as(timeline, Mesh3D::descriptor_albedo_factor());
                let all_albedo_buffers =
                    results.iter_as(timeline, Mesh3D::descriptor_albedo_texture_buffer());
                let all_albedo_formats =
                    results.iter_as(timeline, Mesh3D::descriptor_albedo_texture_format());

                let query_result_hash = results.query_result_hash();

                let data = re_query::range_zip_1x7(
                    all_vertex_positions_indexed,
                    all_vertex_normals.slice::<[f32; 3]>(),
                    all_vertex_colors.slice::<u32>(),
                    all_vertex_texcoords.slice::<[f32; 2]>(),
                    all_triangle_indices.slice::<[u32; 3]>(),
                    all_albedo_factors.slice::<u32>(),
                    all_albedo_buffers.slice::<&[u8]>(),
                    // Legit call to `component_slow`, `ImageFormat` is real complicated.
                    all_albedo_formats.component_slow::<ImageFormat>(),
                )
                .map(
                    |(
                        index,
                        vertex_positions,
                        vertex_normals,
                        vertex_colors,
                        vertex_texcoords,
                        triangle_indices,
                        albedo_factors,
                        albedo_buffers,
                        albedo_formats,
                    )| {
                        Mesh3DComponentData {
                            index,
                            query_result_hash,

                            // Note that we rely in clamping in the mesh loader for some of these.
                            // (which means that if we have a mesh cache hit, we don't have to bother with clamping logic here!)
                            native_mesh: NativeMesh3D {
                                vertex_positions: bytemuck::cast_slice(vertex_positions),
                                vertex_normals: vertex_normals.map(bytemuck::cast_slice),
                                vertex_colors: vertex_colors.map(bytemuck::cast_slice),
                                vertex_texcoords: vertex_texcoords.map(bytemuck::cast_slice),
                                triangle_indices: triangle_indices.map(bytemuck::cast_slice),
                                albedo_factor: albedo_factors
                                    .map(bytemuck::cast_slice)
                                    .and_then(|albedo_factors| albedo_factors.first().copied()),
                                albedo_texture_buffer: albedo_buffers
                                    .unwrap_or_default()
                                    .first()
                                    .cloned()
                                    .map(Into::into), // shallow clone
                                albedo_texture_format: albedo_formats
                                    .unwrap_or_default()
                                    .first()
                                    .map(|format| format.0),
                            },
                        }
                    },
                );

                self.process_data(ctx, ctx.render_ctx(), &mut instances, spatial_ctx, data);

                Ok(())
            },
        )?;

        match re_renderer::renderer::MeshDrawData::new(ctx.viewer_ctx.render_ctx(), &instances) {
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

    fn fallback_provider(&self) -> &dyn re_viewer_context::ComponentFallbackProvider {
        self
    }
}

re_viewer_context::impl_component_fallback_provider!(Mesh3DVisualizer => []);
