use re_chunk_store::RowId;
use re_log_types::{hash::Hash64, Instance, TimeInt};
use re_renderer::{renderer::GpuMeshInstance, RenderContext};
use re_types::{
    archetypes::Mesh3D,
    components::{
        AlbedoFactor, ClassId, Color, ImageBuffer, ImageFormat, Position3D, Texcoord2D,
        TriangleIndices, Vector3D,
    },
    Component as _,
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem, QueryContext, ViewContext, ViewContextCollection,
    ViewQuery, ViewSystemExecutionError, VisualizableEntities, VisualizableFilterContext,
    VisualizerQueryInfo, VisualizerSystem,
};

use super::{
    entity_iterator::clamped_vec_or_empty, filter_visualizable_3d_entities,
    SpatialViewVisualizerData,
};

use crate::{
    contexts::SpatialSceneEntityContext,
    mesh_cache::{AnyMesh, MeshCache, MeshCacheKey},
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

    vertex_positions: &'a [Position3D],
    vertex_normals: &'a [Vector3D],
    vertex_colors: &'a [Color],
    vertex_texcoords: &'a [Texcoord2D],

    triangle_indices: Option<&'a [TriangleIndices]>,
    albedo_factor: Option<&'a AlbedoFactor>,
    albedo_buffer: Option<ImageBuffer>,
    albedo_format: Option<ImageFormat>,

    class_ids: &'a [ClassId],
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
            if data.vertex_positions.is_empty() {
                continue;
            }

            let mesh = ctx.viewer_ctx.cache.entry(|c: &mut MeshCache| {
                let key = MeshCacheKey {
                    versioned_instance_path_hash: picking_instance_hash.versioned(primary_row_id),
                    query_result_hash: data.query_result_hash,
                    media_type: None,
                };

                let vertex_normals =
                    clamped_vec_or_empty(data.vertex_normals, data.vertex_positions.len());
                let vertex_colors =
                    clamped_vec_or_empty(data.vertex_colors, data.vertex_positions.len());
                let vertex_texcoords =
                    clamped_vec_or_empty(data.vertex_texcoords, data.vertex_positions.len());

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
                            albedo_factor: data.albedo_factor.copied(),
                            albedo_texture_buffer: data.albedo_buffer.clone(), // shallow clone
                            albedo_texture_format: data.albedo_format,
                            class_ids: (!data.class_ids.is_empty())
                                .then(|| data.class_ids.to_owned()),
                        },
                        texture_key: re_log_types::hash::Hash64::hash(&key).hash64(),
                    },
                    render_ctx,
                )
            });

            if let Some(mesh) = mesh {
                // Let's draw the mesh once for every instance transform.
                // TODO(#7026): We should formalize this kind of hybrid joining better.
                for &world_from_instance in &ent_context.transform_info.reference_from_instances {
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
        entities: ApplicableEntities,
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
        let Some(render_ctx) = ctx.viewer_ctx.render_ctx else {
            return Err(ViewSystemExecutionError::NoRenderContextError);
        };

        let mut instances = Vec::new();

        use super::entity_iterator::{iter_primitive_array, process_archetype};
        process_archetype::<Self, Mesh3D, _>(
            ctx,
            view_query,
            context_systems,
            |ctx, spatial_ctx, results| {
                use re_view::RangeResultsExt as _;

                let Some(all_vertex_position_chunks) =
                    results.get_required_chunks(&Position3D::name())
                else {
                    return Ok(());
                };

                let timeline = ctx.query.timeline();
                let all_vertex_positions_indexed = iter_primitive_array::<3, f32>(
                    &all_vertex_position_chunks,
                    timeline,
                    Position3D::name(),
                );
                let all_vertex_normals = results.iter_as(timeline, Vector3D::name());
                let all_vertex_colors = results.iter_as(timeline, Color::name());
                let all_vertex_texcoords = results.iter_as(timeline, Texcoord2D::name());
                let all_triangle_indices = results.iter_as(timeline, TriangleIndices::name());
                let all_albedo_factors = results.iter_as(timeline, AlbedoFactor::name());
                let all_albedo_buffers = results.iter_as(timeline, ImageBuffer::name());
                let all_albedo_formats = results.iter_as(timeline, ImageFormat::name());
                let all_class_ids = results.iter_as(timeline, ClassId::name());

                let query_result_hash = results.query_result_hash();

                let data = re_query::range_zip_1x8(
                    all_vertex_positions_indexed,
                    all_vertex_normals.primitive_array::<3, f32>(),
                    all_vertex_colors.primitive::<u32>(),
                    all_vertex_texcoords.primitive_array::<2, f32>(),
                    all_triangle_indices.primitive_array::<3, u32>(),
                    all_albedo_factors.primitive::<u32>(),
                    // TODO(cmc): Provide a `iter_blob`.
                    all_albedo_buffers.component_slow::<ImageBuffer>(),
                    // Legit call to `component_slow`, `ImageFormat` is real complicated.
                    all_albedo_formats.component_slow::<ImageFormat>(),
                    all_class_ids.primitive::<u16>(),
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
                        class_ids,
                    )| {
                        Mesh3DComponentData {
                            index,
                            query_result_hash,
                            vertex_positions: bytemuck::cast_slice(vertex_positions),
                            vertex_normals: vertex_normals
                                .map_or(&[], |vertex_normals| bytemuck::cast_slice(vertex_normals)),
                            vertex_colors: vertex_colors
                                .map_or(&[], |vertex_colors| bytemuck::cast_slice(vertex_colors)),
                            vertex_texcoords: vertex_texcoords.map_or(&[], |vertex_texcoords| {
                                bytemuck::cast_slice(vertex_texcoords)
                            }),
                            triangle_indices: triangle_indices.map(bytemuck::cast_slice),
                            albedo_factor: albedo_factors
                                .map_or(&[] as &[AlbedoFactor], |albedo_factors| {
                                    bytemuck::cast_slice(albedo_factors)
                                })
                                .first(),
                            albedo_buffer: albedo_buffers.unwrap_or_default().first().cloned(), // shallow clone
                            albedo_format: albedo_formats.unwrap_or_default().first().copied(),
                            class_ids: class_ids
                                .map_or(&[], |class_ids| bytemuck::cast_slice(class_ids)),
                        }
                    },
                );

                self.process_data(ctx, render_ctx, &mut instances, spatial_ctx, data);

                Ok(())
            },
        )?;

        match re_renderer::renderer::MeshDrawData::new(render_ctx, &instances) {
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
