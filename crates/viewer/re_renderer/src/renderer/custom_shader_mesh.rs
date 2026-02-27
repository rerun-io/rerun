//! Custom shader mesh renderer.
//!
//! Renders mesh geometry with a user-provided WGSL fragment shader.
//! Reuses the standard mesh vertex buffers and instance data layout
//! from the regular mesh renderer, but replaces the fragment shader.
//!
//! This is an experimental feature.

use std::ops::Range;
use std::sync::Arc;

use smallvec::smallvec;

use super::mesh_renderer::gpu_data as instance_gpu_data;
use super::{DrawData, DrawError, RenderContext, Renderer};
use crate::draw_phases::{DrawPhase, OutlineMaskProcessor};
use crate::mesh::{GpuMesh, mesh_vertices};
use crate::renderer::{DrawDataDrawable, DrawInstruction, DrawableCollectionViewInfo};
use crate::view_builder::ViewBuilder;
use crate::wgpu_resources::{
    BufferDesc, GpuBuffer, GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor,
    GpuShaderModuleHandle, PipelineLayoutDesc, RenderPipelineDesc,
};
use crate::{
    Color32, CpuWriteGpuReadError, DrawableCollector, OutlineMaskPreference, PickingLayerId,
    PickingLayerProcessor, include_shader_module,
};

/// A single instance of a mesh rendered with a custom shader.
#[derive(Clone)]
pub struct CustomShaderMeshInstance {
    /// The GPU mesh to render.
    pub gpu_mesh: Arc<GpuMesh>,

    /// Transform from mesh space to world space.
    pub world_from_mesh: glam::Affine3A,

    /// Outline mask for selection highlighting.
    pub outline_mask_ids: OutlineMaskPreference,

    /// Picking layer ID for mouse interaction.
    pub picking_layer_id: PickingLayerId,

    /// Additive tint color.
    pub additive_tint: Color32,

    /// The user's custom WGSL fragment shader handle.
    pub shader_module: GpuShaderModuleHandle,

    /// Hash of the shader source for pipeline caching.
    pub shader_hash: u64,

    /// Optional custom bind group for shader parameters.
    /// Bound at group 2 (group 0 = global, group 1 = mesh material).
    pub custom_bind_group: Option<Arc<wgpu::BindGroup>>,

    /// Optional custom bind group layout (must match `custom_bind_group`).
    pub custom_bind_group_layout: Option<Arc<wgpu::BindGroupLayout>>,
}

/// Create an inline shader module from WGSL source, cached by content hash.
pub fn create_custom_shader_module(
    ctx: &RenderContext,
    label: &str,
    wgsl_source: &str,
) -> (GpuShaderModuleHandle, u64) {
    use std::hash::{Hash as _, Hasher as _};
    let mut hasher = ahash::AHasher::default();
    wgsl_source.hash(&mut hasher);
    let content_hash = hasher.finish();

    let handle = ctx.gpu_resources.shader_modules.get_or_create_inline(
        &ctx.device,
        label,
        wgsl_source,
        content_hash,
    );

    (handle, content_hash)
}

/// Returns the draw phases that a custom shader mesh participates in.
pub fn custom_shader_draw_phases() -> enumset::EnumSet<DrawPhase> {
    DrawPhase::Opaque | DrawPhase::PickingLayer | DrawPhase::OutlineMask
}

// ---

/// Pipeline handles for a specific custom shader variant.
#[derive(Clone)]
struct PipelineSet {
    rp_shaded: GpuRenderPipelineHandle,
    rp_picking_layer: GpuRenderPipelineHandle,
    rp_outline_mask: GpuRenderPipelineHandle,
}

/// A batch of custom shader mesh instances drawn together.
#[derive(Clone)]
struct CustomMeshBatch {
    mesh: Arc<GpuMesh>,
    instance_range: Range<u32>,
    draw_phase: DrawPhase,
    position: glam::Vec3A,

    /// Index into the pipeline/bind-group arrays.
    group_idx: usize,
}

/// Per-shader-group data stored in the draw data.
#[derive(Clone)]
struct ShaderGroup {
    pipelines: PipelineSet,
    custom_bind_group: Option<Arc<wgpu::BindGroup>>,
}

#[derive(Clone)]
pub struct CustomShaderMeshDrawData {
    instance_buffer: Option<GpuBuffer>,
    batches: Vec<CustomMeshBatch>,
    shader_groups: Vec<ShaderGroup>,
}

impl DrawData for CustomShaderMeshDrawData {
    type Renderer = CustomShaderMeshRenderer;

    fn collect_drawables(
        &self,
        view_info: &DrawableCollectionViewInfo,
        collector: &mut DrawableCollector<'_>,
    ) {
        for (batch_idx, batch) in self.batches.iter().enumerate() {
            collector.add_drawable_for_phase(
                batch.draw_phase,
                DrawDataDrawable::from_world_position(view_info, batch.position, batch_idx as _),
            );
        }
    }
}

impl CustomShaderMeshDrawData {
    pub fn new(
        ctx: &RenderContext,
        instances: &[CustomShaderMeshInstance],
    ) -> Result<Self, CpuWriteGpuReadError> {
        re_tracing::profile_function!();

        if instances.is_empty() {
            return Ok(Self {
                batches: Vec::new(),
                instance_buffer: None,
                shader_groups: Vec::new(),
            });
        }

        // Get the standard shader module (for vertex shader + picking/outline fragments).
        let standard_shader_module = ctx.gpu_resources.shader_modules.get_or_create(
            ctx,
            &include_shader_module!("../../shader/instanced_mesh.wgsl"),
        );

        // Get the mesh renderer's material bind group layout for group 1.
        let mesh_renderer = ctx.renderer::<super::mesh_renderer::MeshRenderer>();
        let material_bind_group_layout = mesh_renderer.bind_group_layout;
        drop(mesh_renderer); // Release read lock

        // Build shader groups: one per unique (shader_hash, bind_group_layout) combination.
        // Each group gets its own pipeline set.
        let mut shader_group_map: ahash::HashMap<u64, usize> = ahash::HashMap::default();
        let mut shader_groups: Vec<ShaderGroup> = Vec::new();

        for instance in instances {
            if shader_group_map.contains_key(&instance.shader_hash) {
                continue;
            }

            // Build pipeline layout: group 0 (global) + group 1 (material) + optional group 2 (custom)
            let mut pipeline_layout_entries = vec![
                ctx.global_bindings.layout, // Group 0
                material_bind_group_layout, // Group 1
            ];

            if let Some(custom_layout) = &instance.custom_bind_group_layout {
                let custom_layout_handle = ctx
                    .gpu_resources
                    .bind_group_layouts
                    .register_existing(custom_layout.as_ref().clone());
                pipeline_layout_entries.push(custom_layout_handle);
            }

            let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
                ctx,
                &PipelineLayoutDesc {
                    label: "CustomShaderMeshRenderer::pipeline_layout".into(),
                    entries: pipeline_layout_entries,
                },
            );

            let front_face = wgpu::FrontFace::Ccw;
            let primitive = wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                front_face,
                ..Default::default()
            };
            let vertex_buffers: smallvec::SmallVec<[_; 4]> =
                std::iter::once(instance_gpu_data::InstanceData::vertex_buffer_layout())
                    .chain(mesh_vertices::vertex_buffer_layouts())
                    .collect();

            let rp_shaded_desc = RenderPipelineDesc {
                label: "CustomShaderMeshRenderer::rp_shaded".into(),
                pipeline_layout,
                vertex_entrypoint: "vs_main".into(),
                vertex_handle: standard_shader_module,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: instance.shader_module,
                vertex_buffers,
                render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into())],
                primitive,
                depth_stencil: Some(ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE),
                multisample: ViewBuilder::main_target_default_msaa_state(
                    ctx.render_config(),
                    false,
                ),
            };
            let rp_shaded = ctx
                .gpu_resources
                .render_pipelines
                .get_or_create(ctx, &rp_shaded_desc);

            let rp_picking_layer = ctx.gpu_resources.render_pipelines.get_or_create(
                ctx,
                &RenderPipelineDesc {
                    label: "CustomShaderMeshRenderer::rp_picking_layer".into(),
                    fragment_entrypoint: "fs_main_picking_layer".into(),
                    fragment_handle: standard_shader_module,
                    render_targets: smallvec![Some(
                        PickingLayerProcessor::PICKING_LAYER_FORMAT.into()
                    )],
                    depth_stencil: PickingLayerProcessor::PICKING_LAYER_DEPTH_STATE,
                    multisample: PickingLayerProcessor::PICKING_LAYER_MSAA_STATE,
                    ..rp_shaded_desc.clone()
                },
            );

            let rp_outline_mask = ctx.gpu_resources.render_pipelines.get_or_create(
                ctx,
                &RenderPipelineDesc {
                    label: "CustomShaderMeshRenderer::rp_outline_mask".into(),
                    fragment_entrypoint: "fs_main_outline_mask".into(),
                    fragment_handle: standard_shader_module,
                    render_targets: smallvec![Some(OutlineMaskProcessor::MASK_FORMAT.into())],
                    depth_stencil: OutlineMaskProcessor::MASK_DEPTH_STATE,
                    multisample: OutlineMaskProcessor::mask_default_msaa_state(
                        ctx.device_caps().tier,
                    ),
                    ..rp_shaded_desc
                },
            );

            let group_idx = shader_groups.len();
            shader_groups.push(ShaderGroup {
                pipelines: PipelineSet {
                    rp_shaded,
                    rp_picking_layer,
                    rp_outline_mask,
                },
                custom_bind_group: instance.custom_bind_group.clone(),
            });
            shader_group_map.insert(instance.shader_hash, group_idx);
        }

        // Allocate instance buffer.
        let instance_buffer_size =
            (std::mem::size_of::<instance_gpu_data::InstanceData>() * instances.len()) as _;
        let instance_buffer = ctx.gpu_resources.buffers.alloc(
            &ctx.device,
            &BufferDesc {
                label: "CustomShaderMeshDrawData::instance_buffer".into(),
                size: instance_buffer_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );

        let mut batches = Vec::new();
        {
            let mut instance_buffer_staging = ctx
                .cpu_write_gpu_read_belt
                .lock()
                .allocate::<instance_gpu_data::InstanceData>(
                &ctx.device,
                &ctx.gpu_resources.buffers,
                instances.len(),
            )?;

            for (i, instance) in instances.iter().enumerate() {
                let world_from_mesh_mat3 = instance.world_from_mesh.matrix3;
                let world_from_mesh_normal =
                    if instance.world_from_mesh.matrix3.determinant() != 0.0 {
                        instance.world_from_mesh.matrix3.inverse().transpose()
                    } else {
                        glam::Mat3A::ZERO
                    };

                instance_buffer_staging.push(instance_gpu_data::InstanceData {
                    world_from_mesh_row_0: world_from_mesh_mat3
                        .row(0)
                        .extend(instance.world_from_mesh.translation.x)
                        .to_array(),
                    world_from_mesh_row_1: world_from_mesh_mat3
                        .row(1)
                        .extend(instance.world_from_mesh.translation.y)
                        .to_array(),
                    world_from_mesh_row_2: world_from_mesh_mat3
                        .row(2)
                        .extend(instance.world_from_mesh.translation.z)
                        .to_array(),
                    world_from_mesh_normal_row_0: world_from_mesh_normal.row(0).to_array(),
                    world_from_mesh_normal_row_1: world_from_mesh_normal.row(1).to_array(),
                    world_from_mesh_normal_row_2: world_from_mesh_normal.row(2).to_array(),
                    additive_tint: instance.additive_tint,
                    outline_mask_ids: instance
                        .outline_mask_ids
                        .0
                        .map_or([0, 0, 0, 0], |mask| [mask[0], mask[1], 0, 0]),
                    picking_layer_id: instance.picking_layer_id.into(),
                })?;

                let mesh_center = glam::Vec3A::from(instance.gpu_mesh.bbox.center());
                let world_pos = instance.world_from_mesh.transform_point3a(mesh_center);
                let instance_idx = i as u32;
                let group_idx = shader_group_map[&instance.shader_hash];

                // Each custom shader instance gets individual batches per phase.
                batches.push(CustomMeshBatch {
                    mesh: instance.gpu_mesh.clone(),
                    instance_range: instance_idx..(instance_idx + 1),
                    draw_phase: DrawPhase::Opaque,
                    position: world_pos,
                    group_idx,
                });
                if instance.outline_mask_ids.is_some() {
                    batches.push(CustomMeshBatch {
                        mesh: instance.gpu_mesh.clone(),
                        instance_range: instance_idx..(instance_idx + 1),
                        draw_phase: DrawPhase::OutlineMask,
                        position: world_pos,
                        group_idx,
                    });
                }
                batches.push(CustomMeshBatch {
                    mesh: instance.gpu_mesh.clone(),
                    instance_range: instance_idx..(instance_idx + 1),
                    draw_phase: DrawPhase::PickingLayer,
                    position: world_pos,
                    group_idx,
                });
            }

            instance_buffer_staging.copy_to_buffer(
                ctx.active_frame.before_view_builder_encoder.lock().get(),
                &instance_buffer,
                0,
            )?;
        }

        Ok(Self {
            batches,
            instance_buffer: Some(instance_buffer),
            shader_groups,
        })
    }
}

// ---

/// Renderer for meshes with user-provided custom fragment shaders.
///
/// This is a thin renderer: all pipeline creation happens in
/// [`CustomShaderMeshDrawData::new`] since it has access to `RenderContext`.
/// The renderer itself just loads the standard `instanced_mesh.wgsl` module.
pub struct CustomShaderMeshRenderer {
    _standard_shader_module: GpuShaderModuleHandle,
}

impl Renderer for CustomShaderMeshRenderer {
    type RendererDrawData = CustomShaderMeshDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        // Preload the standard shader module so it's available for pipeline creation.
        let standard_shader_module = ctx.gpu_resources.shader_modules.get_or_create(
            ctx,
            &include_shader_module!("../../shader/instanced_mesh.wgsl"),
        );

        Self {
            _standard_shader_module: standard_shader_module,
        }
    }

    fn draw(
        &self,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_instructions: &[DrawInstruction<'_, Self::RendererDrawData>],
    ) -> Result<(), DrawError> {
        re_tracing::profile_function!();

        for DrawInstruction {
            draw_data,
            drawables,
        } in draw_instructions
        {
            let Some(instance_buffer) = &draw_data.instance_buffer else {
                continue;
            };
            pass.set_vertex_buffer(0, instance_buffer.slice(..));

            for drawable in *drawables {
                let batch = &draw_data.batches[drawable.draw_data_payload as usize];
                let group = &draw_data.shader_groups[batch.group_idx];

                // Select pipeline based on phase.
                let pipeline_handle = match phase {
                    DrawPhase::Opaque => group.pipelines.rp_shaded,
                    DrawPhase::PickingLayer => group.pipelines.rp_picking_layer,
                    DrawPhase::OutlineMask => group.pipelines.rp_outline_mask,
                    _ => {
                        unreachable!(
                            "CustomShaderMeshRenderer called on unexpected phase: {phase:?}"
                        )
                    }
                };
                pass.set_pipeline(render_pipelines.get(pipeline_handle)?);

                // Bind mesh vertex buffers.
                let vertex_buffer_combined = &batch.mesh.vertex_buffer_combined;
                let index_buffer = &batch.mesh.index_buffer;

                pass.set_vertex_buffer(
                    1,
                    vertex_buffer_combined.slice(batch.mesh.vertex_buffer_positions_range.clone()),
                );
                pass.set_vertex_buffer(
                    2,
                    vertex_buffer_combined.slice(batch.mesh.vertex_buffer_colors_range.clone()),
                );
                pass.set_vertex_buffer(
                    3,
                    vertex_buffer_combined.slice(batch.mesh.vertex_buffer_normals_range.clone()),
                );
                pass.set_vertex_buffer(
                    4,
                    vertex_buffer_combined.slice(batch.mesh.vertex_buffer_texcoord_range.clone()),
                );
                pass.set_index_buffer(
                    index_buffer.slice(batch.mesh.index_buffer_range.clone()),
                    wgpu::IndexFormat::Uint32,
                );

                // Bind custom bind group at group 2 (if present).
                if let Some(custom_bind_group) = &group.custom_bind_group {
                    pass.set_bind_group(2, custom_bind_group.as_ref(), &[]);
                }

                // Draw each material's index range.
                for material in &batch.mesh.materials {
                    pass.set_bind_group(1, &material.bind_group, &[]);
                    pass.draw_indexed(
                        material.index_range.clone(),
                        0,
                        batch.instance_range.clone(),
                    );
                }
            }
        }

        Ok(())
    }
}
