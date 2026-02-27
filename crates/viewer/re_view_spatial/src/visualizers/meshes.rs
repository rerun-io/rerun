use std::sync::Arc;

use re_chunk_store::RowId;
use re_entity_db::EntityPath;
use re_log_types::hash::Hash64;
use re_log_types::{Instance, TimeInt};
use re_renderer::external::wgpu;
use re_renderer::renderer::{
    create_custom_shader_module, CustomShaderMeshInstance, GpuMeshInstance,
};
use re_renderer::RenderContext;
use re_sdk_types::archetypes::Mesh3D;
use re_sdk_types::components::{ImageFormat, Scalar, ShaderParameters, ShaderSource};
use re_sdk_types::Archetype as _;
use re_viewer_context::{
    IdentifiedViewSystem, QueryContext, ViewContext, ViewContextCollection, ViewQuery,
    ViewSystemExecutionError, VisualizerExecutionOutput, VisualizerQueryInfo, VisualizerSystem,
};

use super::SpatialViewVisualizerData;
use crate::caches::{AnyMesh, MeshCache, MeshCacheKey};
use crate::contexts::SpatialSceneVisualizerInstructionContext;
use crate::custom_shader_bind_group::build_custom_bind_group;
use crate::mesh_loader::NativeMesh3D;
use crate::shader_param_resolver::resolve_shader_params;
use crate::shader_params::ShaderParametersMeta;
use crate::view_kind::SpatialViewKind;

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
    shader_source: Option<String>,
    shader_parameters: Option<String>,
}

// NOTE: Do not put profile scopes in these methods. They are called for all entities and all
// timestamps within a time range -- it's _a lot_.
impl Mesh3DVisualizer {
    fn process_data<'a>(
        &mut self,
        ctx: &QueryContext<'_>,
        render_ctx: &RenderContext,
        instances: &mut Vec<GpuMeshInstance>,
        custom_instances: &mut Vec<CustomShaderMeshInstance>,
        ent_context: &SpatialSceneVisualizerInstructionContext<'_>,
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

            let Some(mesh) = mesh else {
                continue;
            };

            // Branch: custom shader path vs standard path.
            if let Some(shader_source) = &data.shader_source {
                // Custom shader path: create CustomShaderMeshInstance.
                let (shader_module, shader_hash) =
                    create_custom_shader_module(render_ctx, "custom_mesh_shader", shader_source);

                // Parse shader parameters and resolve values from the store.
                let (custom_bind_group, custom_bind_group_layout) = self
                    .resolve_custom_shader_bindings(
                        ctx,
                        render_ctx,
                        entity_path,
                        data.shader_parameters.as_deref(),
                    );

                for &world_from_instance in ent_context.transform_info.target_from_instances() {
                    let world_from_instance = world_from_instance.as_affine3a();
                    let bind_group = custom_bind_group.clone();
                    let bind_group_layout = custom_bind_group_layout.clone();
                    custom_instances.extend(mesh.mesh_instances.iter().map(move |mesh_instance| {
                        let entity_from_mesh = mesh_instance.world_from_mesh;
                        let world_from_mesh = world_from_instance * entity_from_mesh;

                        CustomShaderMeshInstance {
                            gpu_mesh: mesh_instance.gpu_mesh.clone(),
                            world_from_mesh,
                            outline_mask_ids,
                            picking_layer_id: re_view::picking_layer_id_from_instance_path_hash(
                                picking_instance_hash,
                            ),
                            additive_tint: re_renderer::Color32::BLACK,
                            shader_module,
                            shader_hash,
                            custom_bind_group: bind_group.clone(),
                            custom_bind_group_layout: bind_group_layout.clone(),
                        }
                    }));

                    self.0
                        .add_bounding_box(entity_path.hash(), mesh.bbox(), world_from_instance);
                }
            } else {
                // Standard path: create GpuMeshInstance (unchanged).
                for &world_from_instance in ent_context.transform_info.target_from_instances() {
                    let world_from_instance = world_from_instance.as_affine3a();
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
                            additive_tint: re_renderer::Color32::BLACK,
                        }
                    }));

                    self.0
                        .add_bounding_box(entity_path.hash(), mesh.bbox(), world_from_instance);
                }
            }
        }
    }

    /// Parse shader parameters JSON, resolve uniform/texture values from the store,
    /// and build the custom bind group for a custom shader mesh.
    #[allow(clippy::unused_self)]
    fn resolve_custom_shader_bindings(
        &self,
        ctx: &QueryContext<'_>,
        render_ctx: &RenderContext,
        mesh_entity: &EntityPath,
        shader_parameters_json: Option<&str>,
    ) -> (
        Option<Arc<wgpu::BindGroup>>,
        Option<Arc<wgpu::BindGroupLayout>>,
    ) {
        let Some(json) = shader_parameters_json else {
            return (None, None);
        };

        let params = match ShaderParametersMeta::from_json(json) {
            Ok(params) => params,
            Err(e) => {
                re_log::warn_once!("Failed to parse shader parameters JSON: {e}");
                return (None, None);
            }
        };

        if params.uniforms.is_empty() && params.textures.is_empty() {
            return (None, None);
        }

        // Resolve scalar/vector uniforms from the store.
        let entity_db = ctx.recording();
        let query = &ctx.query;

        let resolve_scalar = |source_entity: &EntityPath| -> Option<f64> {
            let results = entity_db.latest_at(
                query,
                source_entity,
                re_sdk_types::archetypes::Scalars::all_component_identifiers(),
            );
            results
                .component_mono::<Scalar>(
                    re_sdk_types::archetypes::Scalars::descriptor_scalars().component,
                )
                .map(|s| s.0 .0)
        };

        let resolve_vec = |source_entity: &EntityPath, count: usize| -> Vec<f64> {
            // Try to read a Tensor logged at this entity to get vector values.
            let results = entity_db.latest_at(
                query,
                source_entity,
                re_sdk_types::archetypes::Tensor::all_component_identifiers(),
            );
            if let Some(tensor_data) = results
                .component_mono::<re_sdk_types::components::TensorData>(
                    re_sdk_types::archetypes::Tensor::descriptor_data().component,
                )
            {
                // Extract flat f64 values from the tensor buffer.
                extract_tensor_f64_values(&tensor_data, count)
            } else {
                // Fallback: try reading as a single scalar.
                if let Some(scalar_val) = resolve_scalar(source_entity) {
                    let mut v = vec![scalar_val];
                    v.resize(count, 0.0);
                    v
                } else {
                    vec![0.0; count]
                }
            }
        };

        let resolved = resolve_shader_params(mesh_entity, &params, &resolve_scalar, &resolve_vec);

        // Upload 3D textures and collect bindings.
        let mut texture_bindings = Vec::new();
        for (binding, source_entity) in &resolved.texture_3d_bindings {
            if let Some(gpu_tex) =
                upload_3d_texture_from_store(ctx, render_ctx, source_entity, query)
            {
                texture_bindings.push((*binding, gpu_tex));
            }
        }

        // Build the bind group.
        match build_custom_bind_group(
            render_ctx,
            &mesh_entity.to_string(),
            &resolved.uniform_data,
            &texture_bindings,
        ) {
            Some(bind_group) => (
                Some(Arc::new(bind_group.bind_group)),
                Some(Arc::new(bind_group.bind_group_layout)),
            ),
            None => (None, None),
        }
    }
}

/// Extract flat f64 values from a `TensorData` component.
fn extract_tensor_f64_values(
    tensor_data: &re_sdk_types::components::TensorData,
    count: usize,
) -> Vec<f64> {
    let buffer = &tensor_data.0.buffer;
    let values: Vec<f64> = match buffer {
        re_sdk_types::datatypes::TensorBuffer::F32(data) => {
            data.iter().take(count).map(|v| *v as f64).collect()
        }
        re_sdk_types::datatypes::TensorBuffer::F64(data) => {
            data.iter().take(count).copied().collect()
        }
        re_sdk_types::datatypes::TensorBuffer::I32(data) => {
            data.iter().take(count).map(|v| *v as f64).collect()
        }
        re_sdk_types::datatypes::TensorBuffer::U16(data) => {
            data.iter().take(count).map(|v| *v as f64).collect()
        }
        _ => vec![0.0; count],
    };

    let mut result = values;
    result.resize(count, 0.0);
    result
}

/// Upload a 3D texture from a `TensorData` component in the store.
fn upload_3d_texture_from_store(
    ctx: &QueryContext<'_>,
    render_ctx: &RenderContext,
    source_entity: &EntityPath,
    query: &re_chunk_store::LatestAtQuery,
) -> Option<re_renderer::resource_managers::GpuTexture3D> {
    let entity_db = ctx.recording();
    let results = entity_db.latest_at(
        query,
        source_entity,
        re_sdk_types::archetypes::Tensor::all_component_identifiers(),
    );

    let tensor_data = results.component_mono::<re_sdk_types::components::TensorData>(
        re_sdk_types::archetypes::Tensor::descriptor_data().component,
    )?;

    let shape = &tensor_data.0.shape;
    if shape.len() != 3 {
        re_log::warn_once!(
            "Expected 3D tensor for volume texture at {source_entity}, got {}D",
            shape.len()
        );
        return None;
    }

    let depth = shape[0] as u32;
    let height = shape[1] as u32;
    let width = shape[2] as u32;

    // Convert tensor data to f32 bytes for GPU upload.
    let (format, data) = tensor_buffer_to_gpu_data(&tensor_data.0.buffer, width, height, depth)?;

    // Use a hash of the entity path + row ID as cache key.
    let row_id =
        results.component_row_id(re_sdk_types::archetypes::Tensor::descriptor_data().component)?;
    let mut hasher = std::hash::BuildHasher::build_hasher(&ahash::RandomState::new());
    std::hash::Hash::hash(&source_entity, &mut hasher);
    std::hash::Hash::hash(&row_id, &mut hasher);
    let cache_key = std::hash::Hasher::finish(&hasher);

    let texture = render_ctx.texture_manager_3d.get_or_create(
        cache_key,
        render_ctx,
        &re_renderer::resource_managers::VolumeDataDesc {
            label: format!("volume_{source_entity}"),
            width,
            height,
            depth,
            format,
            data: &data,
        },
    );

    Some(texture)
}

/// Convert a tensor buffer to GPU-compatible bytes, returning (format, data).
fn tensor_buffer_to_gpu_data(
    buffer: &re_sdk_types::datatypes::TensorBuffer,
    width: u32,
    height: u32,
    depth: u32,
) -> Option<(wgpu::TextureFormat, Vec<u8>)> {
    let expected_elements = (width as usize) * (height as usize) * (depth as usize);

    match buffer {
        re_sdk_types::datatypes::TensorBuffer::F32(data) => {
            if data.len() < expected_elements {
                re_log::warn_once!(
                    "F32 tensor buffer too small: expected {expected_elements}, got {}",
                    data.len()
                );
                return None;
            }
            let bytes: Vec<u8> = data[..expected_elements]
                .iter()
                .flat_map(|v| v.to_le_bytes())
                .collect();
            Some((wgpu::TextureFormat::R32Float, bytes))
        }
        re_sdk_types::datatypes::TensorBuffer::U16(data) => {
            // Convert u16 to f32 for GPU.
            if data.len() < expected_elements {
                return None;
            }
            let bytes: Vec<u8> = data[..expected_elements]
                .iter()
                .flat_map(|v| (*v as f32).to_le_bytes())
                .collect();
            Some((wgpu::TextureFormat::R32Float, bytes))
        }
        re_sdk_types::datatypes::TensorBuffer::I16(data) => {
            if data.len() < expected_elements {
                return None;
            }
            let bytes: Vec<u8> = data[..expected_elements]
                .iter()
                .flat_map(|v| (*v as f32).to_le_bytes())
                .collect();
            Some((wgpu::TextureFormat::R32Float, bytes))
        }
        _ => {
            re_log::warn_once!("Unsupported tensor buffer type for 3D texture upload");
            None
        }
    }
}

impl IdentifiedViewSystem for Mesh3DVisualizer {
    fn identifier() -> re_viewer_context::ViewSystemIdentifier {
        "Mesh3D".into()
    }
}

impl VisualizerSystem for Mesh3DVisualizer {
    fn visualizer_query_info(
        &self,
        _app_options: &re_viewer_context::AppOptions,
    ) -> VisualizerQueryInfo {
        VisualizerQueryInfo::from_archetype::<Mesh3D>()
    }

    fn execute(
        &mut self,
        ctx: &ViewContext<'_>,
        view_query: &ViewQuery<'_>,
        context_systems: &ViewContextCollection,
    ) -> Result<VisualizerExecutionOutput, ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let output = VisualizerExecutionOutput::default();
        let mut instances = Vec::new();
        let mut custom_instances = Vec::new();

        use super::entity_iterator::process_archetype;
        process_archetype::<Self, Mesh3D, _>(
            ctx,
            view_query,
            context_systems,
            &output,
            self.0.preferred_view_kind,
            |ctx, spatial_ctx, results| {
                let all_vertex_positions =
                    results.iter_required(Mesh3D::descriptor_vertex_positions().component);
                if all_vertex_positions.is_empty() {
                    return Ok(());
                }
                let all_vertex_normals =
                    results.iter_optional(Mesh3D::descriptor_vertex_normals().component);
                let all_vertex_colors =
                    results.iter_optional(Mesh3D::descriptor_vertex_colors().component);
                let all_vertex_texcoords =
                    results.iter_optional(Mesh3D::descriptor_vertex_texcoords().component);
                let all_triangle_indices =
                    results.iter_optional(Mesh3D::descriptor_triangle_indices().component);
                let all_albedo_factors =
                    results.iter_optional(Mesh3D::descriptor_albedo_factor().component);
                let all_albedo_buffers =
                    results.iter_optional(Mesh3D::descriptor_albedo_texture_buffer().component);
                let all_albedo_formats =
                    results.iter_optional(Mesh3D::descriptor_albedo_texture_format().component);
                let all_shader_sources =
                    results.iter_optional(Mesh3D::descriptor_shader_source().component);
                let all_shader_parameters =
                    results.iter_optional(Mesh3D::descriptor_shader_parameters().component);

                let query_result_hash = results.query_result_hash();

                let data = re_query::range_zip_1x9(
                    all_vertex_positions.slice::<[f32; 3]>(),
                    all_vertex_normals.slice::<[f32; 3]>(),
                    all_vertex_colors.slice::<u32>(),
                    all_vertex_texcoords.slice::<[f32; 2]>(),
                    all_triangle_indices.slice::<[u32; 3]>(),
                    all_albedo_factors.slice::<u32>(),
                    all_albedo_buffers.slice::<&[u8]>(),
                    // Legit call to `component_slow`, `ImageFormat` is real complicated.
                    all_albedo_formats.component_slow::<ImageFormat>(),
                    // Legit call to `component_slow`, string-based components.
                    all_shader_sources.component_slow::<ShaderSource>(),
                    all_shader_parameters.component_slow::<ShaderParameters>(),
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
                        shader_sources,
                        shader_parameters,
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
                            shader_source: shader_sources
                                .and_then(|s| s.first().map(|v| v.0.to_string())),
                            shader_parameters: shader_parameters
                                .and_then(|s| s.first().map(|v| v.0.to_string())),
                        }
                    },
                );

                self.process_data(
                    ctx,
                    ctx.render_ctx(),
                    &mut instances,
                    &mut custom_instances,
                    spatial_ctx,
                    data,
                );

                Ok(())
            },
        )?;

        let render_ctx = ctx.viewer_ctx.render_ctx();

        // Build draw data for both standard and custom shader meshes.
        let mut draw_data: Vec<re_renderer::QueueableDrawData> = Vec::new();

        draw_data.push(re_renderer::renderer::MeshDrawData::new(render_ctx, &instances)?.into());

        if !custom_instances.is_empty() {
            draw_data.push(
                re_renderer::renderer::CustomShaderMeshDrawData::new(
                    render_ctx,
                    &custom_instances,
                )?
                .into(),
            );
        }

        Ok(output.with_draw_data(draw_data))
    }

    fn data(&self) -> Option<&dyn std::any::Any> {
        Some(self.0.as_any())
    }
}
