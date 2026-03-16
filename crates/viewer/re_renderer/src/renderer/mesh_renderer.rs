//! Mesh renderer.
//!
//! Uses instancing to render instances of the same mesh in a single draw call.
//! Instance data is kept in an instance-stepped vertex data.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use enumset::EnumSet;
use smallvec::smallvec;

use super::{DrawData, DrawError, RenderContext, Renderer};
use crate::draw_phases::{DrawPhase, OutlineMaskProcessor};
use crate::mesh::gpu_data::MaterialUniformBuffer;
use crate::mesh::{GpuMesh, mesh_vertices};
use crate::renderer::{DrawDataDrawable, DrawInstruction, DrawableCollectionViewInfo};
use crate::view_builder::ViewBuilder;
use crate::wgpu_resources::{
    BindGroupLayoutDesc, BufferDesc, GpuBindGroupLayoutHandle, GpuBuffer, GpuRenderPipelineHandle,
    GpuRenderPipelinePoolAccessor, PipelineLayoutDesc, RenderPipelineDesc,
};
use crate::{
    Color32, CpuWriteGpuReadError, DrawableCollector, OutlineMaskPreference, PickingLayerId,
    PickingLayerProcessor, include_shader_module,
};

mod gpu_data {
    use ecolor::Color32;

    use crate::mesh::mesh_vertices;
    use crate::wgpu_resources::VertexBufferLayout;

    /// Element in the gpu residing instance buffer.
    ///
    /// Keep in sync with `mesh_vertex.wgsl`
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct InstanceData {
        // Don't use aligned glam types because they enforce alignment.
        // (staging buffer might be 4 byte aligned only!)
        pub world_from_mesh_row_0: [f32; 4],
        pub world_from_mesh_row_1: [f32; 4],
        pub world_from_mesh_row_2: [f32; 4],

        pub world_from_mesh_normal_row_0: [f32; 3],
        pub world_from_mesh_normal_row_1: [f32; 3],
        pub world_from_mesh_normal_row_2: [f32; 3],

        pub additive_tint: Color32,

        pub picking_layer_id: [u32; 4],

        // Need only the first two bytes, but we want to keep everything aligned to at least 4 bytes.
        pub outline_mask_ids: [u8; 4],
    }

    impl InstanceData {
        pub fn vertex_buffer_layout() -> VertexBufferLayout {
            let shader_start_location = mesh_vertices::next_free_shader_location();

            VertexBufferLayout {
                array_stride: std::mem::size_of::<Self>() as _,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: VertexBufferLayout::attributes_from_formats(
                    shader_start_location,
                    [
                        // Affine mesh transform.
                        wgpu::VertexFormat::Float32x4,
                        wgpu::VertexFormat::Float32x4,
                        wgpu::VertexFormat::Float32x4,
                        // Transposed inverse mesh transform.
                        wgpu::VertexFormat::Float32x3,
                        wgpu::VertexFormat::Float32x3,
                        wgpu::VertexFormat::Float32x3,
                        // Tint color
                        wgpu::VertexFormat::Unorm8x4,
                        // Picking id.
                        // Again this adds overhead for non-picking passes, more this time. Consider moving this elsewhere.
                        wgpu::VertexFormat::Uint32x4,
                        // Outline mask.
                        // This adds a tiny bit of overhead to all instances during non-outline pass, but the alternative is having yet another vertex buffer.
                        wgpu::VertexFormat::Uint8x2,
                    ]
                    .into_iter(),
                ),
            }
        }
    }
}

/// A batch of mesh instances that are drawn together.
///
/// Note that we don't split the mesh by material.
/// This means that some materials during opaque/transparent drawing need to be ignored.
#[derive(Clone)]
struct MeshBatch {
    mesh: Arc<GpuMesh>,
    instance_range: Range<u32>,
    draw_phase: DrawPhase,

    /// If true, all the instances in this batch have a transparent tint,
    /// meaning that all materials are drawn with transparency.
    /// This can only ever be true if [`Self::draw_phase`] is [`DrawPhase::Transparent`].
    has_transparent_tint: bool,

    /// Controls face culling for this batch during opaque/transparent drawing.
    /// `None` means no culling (show both faces), matching `wgpu::PrimitiveState::cull_mode`.
    cull_mode: Option<wgpu::Face>,

    /// Position of the batch in world space, used for distance sorting.
    position: glam::Vec3A,
}

#[derive(Clone)]
pub struct MeshDrawData {
    // There is a single instance buffer for all instances of all meshes.
    // This means we only ever need to bind the instance buffer once and then change the
    // instance range on every instanced draw call!
    instance_buffer: Option<GpuBuffer>,
    batches: Vec<MeshBatch>,
}

impl DrawData for MeshDrawData {
    type Renderer = MeshRenderer;

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

#[derive(Clone)]
pub struct GpuMeshInstance {
    /// Gpu mesh used by this instance
    pub gpu_mesh: Arc<GpuMesh>,

    /// Where this instance is placed in world space and how its oriented & scaled.
    pub world_from_mesh: glam::Affine3A,

    /// Per-instance (as opposed to per-material/mesh!) tint color that is added to the albedo texture.
    /// The alpha channel is multiplied with the output color.
    pub additive_tint: Color32,

    /// Optional outline mask setting for this instance.
    pub outline_mask_ids: OutlineMaskPreference,

    /// Picking layer id.
    pub picking_layer_id: PickingLayerId,

    /// Controls face culling for this instance.
    /// `None` means no culling (show both faces), matching `wgpu::PrimitiveState::cull_mode`.
    pub cull_mode: Option<wgpu::Face>,
}

impl GpuMeshInstance {
    /// Creates a new instance of a mesh with all fields set to default except for required ones.
    pub fn new(gpu_mesh: Arc<GpuMesh>) -> Self {
        Self {
            gpu_mesh,
            world_from_mesh: glam::Affine3A::IDENTITY,
            additive_tint: Color32::BLACK,
            outline_mask_ids: OutlineMaskPreference::NONE,
            picking_layer_id: PickingLayerId::default(),
            cull_mode: None,
        }
    }
}

/// Batch key: all properties that interrupt instancing.
/// Instances sharing the same batch key can be draw their individual meshes in a single instanced draw call.
#[derive(Clone, Copy, PartialEq, Eq)]
struct BatchKey {
    mesh_ptr: *const GpuMesh,
    cull_mode: Option<wgpu::Face>,
}

impl PartialOrd for BatchKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BatchKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let Self {
            mesh_ptr,
            cull_mode,
        } = self;

        fn face_to_u32(face: Option<wgpu::Face>) -> u32 {
            match face {
                // All things equal, draw back faces first, then no-culling and then front faces.
                // (this is a _cull_ mode, so it specifies which faces to cull, not which to draw!)
                Some(wgpu::Face::Front) => 0,
                None => 1,
                Some(wgpu::Face::Back) => 2,
            }
        }

        mesh_ptr
            .cmp(&other.mesh_ptr)
            .then_with(|| face_to_u32(*cull_mode).cmp(&face_to_u32(other.cull_mode)))
    }
}

impl MeshDrawData {
    /// Transforms and uploads mesh instance data to be consumed by gpu.
    ///
    /// Tries bundling all mesh instances into a single draw data instance whenever possible.
    /// If you pass zero mesh instances, subsequent drawing will do nothing.
    /// Mesh data itself is gpu uploaded if not already present.
    pub fn new(
        ctx: &RenderContext,
        instances: &[GpuMeshInstance],
    ) -> Result<Self, CpuWriteGpuReadError> {
        re_tracing::profile_function!();

        if instances.is_empty() {
            return Ok(Self {
                batches: Vec::new(),
                instance_buffer: None,
            });
        }

        // Group by mesh to facilitate instancing.

        // TODO(andreas): Use a temp allocator
        let instance_buffer_size =
            (std::mem::size_of::<gpu_data::InstanceData>() * instances.len()) as _;
        let instance_buffer = ctx.gpu_resources.buffers.alloc(
            &ctx.device,
            &BufferDesc {
                label: "MeshDrawData::instance_buffer".into(),
                size: instance_buffer_size,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        );

        // NOTE: can't use HashMap here or we get undeterrministic rendering order.
        // See <https://github.com/rerun-io/rerun/issues/10116> for more.
        // Using a `BTreeMap` at least gives the same order every frame,
        // but since it uses the pointer address as part of the key,
        // it will still change if we run the app multiple times.
        let mut instances_by_batch_key: BTreeMap<BatchKey, Vec<_>> = BTreeMap::new();
        for instance in instances {
            instances_by_batch_key
                .entry(BatchKey {
                    mesh_ptr: Arc::as_ptr(&instance.gpu_mesh),
                    cull_mode: instance.cull_mode,
                })
                .or_insert_with(|| Vec::with_capacity(instances.len()))
                .push((instance, EnumSet::<DrawPhase>::new())); // Draw phase is filled out later.
        }

        let mut batches = Vec::new();
        {
            let mut instance_buffer_staging = ctx
                .cpu_write_gpu_read_belt
                .lock()
                .allocate::<gpu_data::InstanceData>(
                &ctx.device,
                &ctx.gpu_resources.buffers,
                instances.len(),
            )?;

            let mut num_processed_instances = 0;
            for (batch_key, mut instances) in instances_by_batch_key {
                let Some(first_instance) = instances.first() else {
                    continue;
                };
                let first_instance = first_instance.0;
                let mesh = first_instance.gpu_mesh.clone();
                let mesh_center = glam::Vec3A::from(mesh.bbox.center());

                // TODO(andreas): precompute these two.
                let any_material_transparent = mesh
                    .materials
                    .iter()
                    .any(|material| material.has_transparency);
                let all_materials_transparent = mesh
                    .materials
                    .iter()
                    .all(|material| material.has_transparency);

                // Any instances participating in the opaque & outline mask drawphases can be batched together.
                // For that, we need continuous runs, ideally all of the instances together in a single run.
                for (instance, phases) in &mut instances {
                    *phases = instance_draw_phases(
                        instance,
                        any_material_transparent,
                        all_materials_transparent,
                    );
                }
                instances.sort_by_key(|(_instance, phases)| *phases);

                // Add the instances to the instance buffer.
                for (i, (instance, phases)) in instances.iter().enumerate() {
                    let world_from_mesh_mat3 = instance.world_from_mesh.matrix3;
                    // If the matrix is not invertible the draw result is likely invalid as well.
                    // However, at this point it's really hard to bail out!
                    // Also, by skipping drawing here, we'd make the result worse as there would be no mesh draw calls that could be debugged.
                    let world_from_mesh_normal =
                        if instance.world_from_mesh.matrix3.determinant() != 0.0 {
                            instance.world_from_mesh.matrix3.inverse().transpose()
                        } else {
                            glam::Mat3A::ZERO
                        };
                    instance_buffer_staging.push(gpu_data::InstanceData {
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

                    // Transparent instances can not be batched.
                    if phases.contains(DrawPhase::Transparent) {
                        let instance_idx = num_processed_instances + i as u32;
                        batches.push(MeshBatch {
                            mesh: mesh.clone(), // TODO(andreas): That's a lot of arc cloning going on here.
                            instance_range: instance_idx..(instance_idx + 1),
                            draw_phase: DrawPhase::Transparent,
                            has_transparent_tint: !instance.additive_tint.is_opaque(),
                            cull_mode: batch_key.cull_mode,
                            position: instance.world_from_mesh.transform_point3a(mesh_center),
                        });
                    }
                }

                // Identify runs of instances with the opaque draw phase for batching.
                // Might be more efficient (citiation needed) to do this in a single iteration, but this is more readable.
                for phase in [DrawPhase::Opaque, DrawPhase::OutlineMask] {
                    let mut instance_start = num_processed_instances;

                    for chunk in instances.chunk_by(|(_, phases_a), (_, phases_b)| {
                        phases_a.contains(phase) == phases_b.contains(phase)
                    }) {
                        let num_instances = chunk.len() as u32;

                        if chunk[0].1.contains(phase) {
                            batches.push(MeshBatch {
                                mesh: mesh.clone(),
                                instance_range: instance_start..(instance_start + num_instances),
                                draw_phase: phase,
                                has_transparent_tint: false,
                                cull_mode: batch_key.cull_mode,
                                // Ordering isn't super important, so for many instances just pick the first as representative.
                                position: chunk[0].0.world_from_mesh.transform_point3a(mesh_center),
                            });
                        }

                        instance_start += num_instances;
                    }
                }

                // Add one additional batch for the picking layer in which all instances are drawn in one go regardless.
                // (see `instance_draw_phases`)
                batches.push(MeshBatch {
                    mesh,
                    instance_range: num_processed_instances
                        ..(num_processed_instances + instances.len() as u32),
                    draw_phase: DrawPhase::PickingLayer,
                    has_transparent_tint: false,
                    cull_mode: batch_key.cull_mode,
                    // Ordering isn't super important, so for many instances just pick the first as representative.
                    position: first_instance
                        .world_from_mesh
                        .transform_point3a(mesh_center),
                });

                num_processed_instances += instances.len() as u32;
            }
            assert_eq!(num_processed_instances as usize, instances.len());
            instance_buffer_staging.copy_to_buffer(
                ctx.active_frame.before_view_builder_encoder.lock().get(),
                &instance_buffer,
                0,
            )?;
        }

        Ok(Self {
            batches,
            instance_buffer: Some(instance_buffer),
        })
    }
}

pub struct MeshRenderer {
    rp_shaded: GpuRenderPipelineHandle,
    rp_shaded_cull_back: GpuRenderPipelineHandle,
    rp_shaded_cull_front: GpuRenderPipelineHandle,

    rp_shaded_alpha_blended_cull_back: GpuRenderPipelineHandle,
    rp_shaded_alpha_blended_cull_front: GpuRenderPipelineHandle,

    rp_picking_layer: GpuRenderPipelineHandle,
    rp_picking_layer_cull_back: GpuRenderPipelineHandle,
    rp_picking_layer_cull_front: GpuRenderPipelineHandle,

    rp_outline_mask: GpuRenderPipelineHandle,
    rp_outline_mask_cull_back: GpuRenderPipelineHandle,
    rp_outline_mask_cull_front: GpuRenderPipelineHandle,

    pub bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for MeshRenderer {
    type RendererDrawData = MeshDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        let render_pipelines = &ctx.gpu_resources.render_pipelines;

        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "MeshRenderer::bind_group_layout".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: (std::mem::size_of::<MaterialUniformBuffer>() as u64)
                                .try_into()
                                .ok(),
                        },
                        count: None,
                    },
                ],
            },
        );
        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &PipelineLayoutDesc {
                label: "MeshRenderer::pipeline_layout".into(),
                entries: vec![ctx.global_bindings.layout, bind_group_layout],
            },
        );

        let shader_module = ctx.gpu_resources.shader_modules.get_or_create(
            ctx,
            &include_shader_module!("../../shader/instanced_mesh.wgsl"),
        );

        // We always assume counter-clockwise faces as front.
        let front_face = wgpu::FrontFace::Ccw;

        let primitive = wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            front_face,
            ..Default::default()
        };
        // Put instance vertex buffer on slot 0 since it doesn't change for several draws.
        let vertex_buffers: smallvec::SmallVec<[_; 4]> =
            std::iter::once(gpu_data::InstanceData::vertex_buffer_layout())
                .chain(mesh_vertices::vertex_buffer_layouts())
                .collect();

        let rp_shaded_desc = RenderPipelineDesc {
            label: "MeshRenderer::rp_shaded".into(),
            pipeline_layout,
            vertex_entrypoint: "vs_main".into(),
            vertex_handle: shader_module,
            fragment_entrypoint: "fs_main_shaded".into(),
            fragment_handle: shader_module,
            vertex_buffers,
            render_targets: smallvec![Some(ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into())],
            primitive,
            depth_stencil: Some(ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE),
            multisample: ViewBuilder::main_target_default_msaa_state(ctx.render_config(), false),
        };
        let rp_shaded = render_pipelines.get_or_create(ctx, &rp_shaded_desc);
        let rp_shaded_cull_back = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "MeshRenderer::rp_shaded_cull_back".into(),
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(wgpu::Face::Back),
                    ..primitive
                },
                ..rp_shaded_desc.clone()
            },
        );
        let rp_shaded_cull_front = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "MeshRenderer::rp_shaded_cull_front".into(),
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(wgpu::Face::Front),
                    ..primitive
                },
                ..rp_shaded_desc.clone()
            },
        );

        let rp_shaded_alpha_blended_cull_back_desc = RenderPipelineDesc {
            label: "MeshRenderer::rp_shaded_alpha_blended_front".into(),
            render_targets: smallvec![Some(wgpu::ColorTargetState {
                format: ViewBuilder::MAIN_TARGET_COLOR_FORMAT,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            depth_stencil: Some(ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE_NO_WRITE),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..primitive
            },
            ..rp_shaded_desc.clone()
        };
        let rp_shaded_alpha_blended_cull_front_desc = RenderPipelineDesc {
            label: "MeshRenderer::rp_shaded_alpha_blended_back".into(),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Front),
                ..primitive
            },
            ..rp_shaded_alpha_blended_cull_back_desc.clone()
        };
        let rp_shaded_alpha_blended_cull_back =
            render_pipelines.get_or_create(ctx, &rp_shaded_alpha_blended_cull_back_desc);
        let rp_shaded_alpha_blended_cull_front =
            render_pipelines.get_or_create(ctx, &rp_shaded_alpha_blended_cull_front_desc);

        let rp_picking_layer_desc = RenderPipelineDesc {
            label: "MeshRenderer::rp_picking_layer".into(),
            fragment_entrypoint: "fs_main_picking_layer".into(),
            render_targets: smallvec![Some(PickingLayerProcessor::PICKING_LAYER_FORMAT.into())],
            depth_stencil: PickingLayerProcessor::PICKING_LAYER_DEPTH_STATE,
            multisample: PickingLayerProcessor::PICKING_LAYER_MSAA_STATE,
            ..rp_shaded_desc.clone()
        };
        let rp_picking_layer = render_pipelines.get_or_create(ctx, &rp_picking_layer_desc);
        let rp_picking_layer_cull_back = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "MeshRenderer::rp_picking_layer_cull_back".into(),
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(wgpu::Face::Back),
                    ..primitive
                },
                ..rp_picking_layer_desc.clone()
            },
        );
        let rp_picking_layer_cull_front = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "MeshRenderer::rp_picking_layer_cull_front".into(),
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(wgpu::Face::Front),
                    ..primitive
                },
                ..rp_picking_layer_desc
            },
        );

        let rp_outline_mask_desc = RenderPipelineDesc {
            label: "MeshRenderer::rp_outline_mask".into(),
            fragment_entrypoint: "fs_main_outline_mask".into(),
            render_targets: smallvec![Some(OutlineMaskProcessor::MASK_FORMAT.into())],
            depth_stencil: OutlineMaskProcessor::MASK_DEPTH_STATE,
            multisample: OutlineMaskProcessor::mask_default_msaa_state(ctx.device_caps().tier),
            ..rp_shaded_desc
        };
        let rp_outline_mask = render_pipelines.get_or_create(ctx, &rp_outline_mask_desc);
        let rp_outline_mask_cull_back = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "MeshRenderer::rp_outline_mask_cull_back".into(),
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(wgpu::Face::Back),
                    ..primitive
                },
                ..rp_outline_mask_desc.clone()
            },
        );
        let rp_outline_mask_cull_front = render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "MeshRenderer::rp_outline_mask_cull_front".into(),
                primitive: wgpu::PrimitiveState {
                    cull_mode: Some(wgpu::Face::Front),
                    ..primitive
                },
                ..rp_outline_mask_desc
            },
        );

        Self {
            rp_shaded,
            rp_shaded_cull_back,
            rp_shaded_cull_front,
            rp_shaded_alpha_blended_cull_back,
            rp_shaded_alpha_blended_cull_front,
            rp_picking_layer,
            rp_picking_layer_cull_back,
            rp_picking_layer_cull_front,
            rp_outline_mask,
            rp_outline_mask_cull_back,
            rp_outline_mask_cull_front,
            bind_group_layout,
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

        match phase {
            DrawPhase::Opaque
            | DrawPhase::Transparent
            | DrawPhase::PickingLayer
            | DrawPhase::OutlineMask => {}
            _ => unreachable!("We were called on a phase we weren't subscribed to: {phase:?}"),
        }

        for DrawInstruction {
            draw_data,
            drawables,
        } in draw_instructions
        {
            let Some(instance_buffer) = &draw_data.instance_buffer else {
                continue; // Instance buffer was empty.
            };
            pass.set_vertex_buffer(0, instance_buffer.slice(..));

            for drawable in *drawables {
                let mesh_batch = &draw_data.batches[drawable.draw_data_payload as usize];

                let vertex_buffer_combined = &mesh_batch.mesh.vertex_buffer_combined;
                let index_buffer = &mesh_batch.mesh.index_buffer;

                pass.set_vertex_buffer(
                    1,
                    vertex_buffer_combined
                        .slice(mesh_batch.mesh.vertex_buffer_positions_range.clone()),
                );
                pass.set_vertex_buffer(
                    2,
                    vertex_buffer_combined
                        .slice(mesh_batch.mesh.vertex_buffer_colors_range.clone()),
                );
                pass.set_vertex_buffer(
                    3,
                    vertex_buffer_combined
                        .slice(mesh_batch.mesh.vertex_buffer_normals_range.clone()),
                );
                pass.set_vertex_buffer(
                    4,
                    vertex_buffer_combined
                        .slice(mesh_batch.mesh.vertex_buffer_texcoord_range.clone()),
                );
                pass.set_index_buffer(
                    index_buffer.slice(mesh_batch.mesh.index_buffer_range.clone()),
                    wgpu::IndexFormat::Uint32,
                );

                // Set per-batch pipeline based on cull mode.
                // For the transparent phase this is done per-material below.
                if phase != DrawPhase::Transparent {
                    let pipeline = match (phase, mesh_batch.cull_mode) {
                        (DrawPhase::Opaque, None) => self.rp_shaded,
                        (DrawPhase::Opaque, Some(wgpu::Face::Back)) => self.rp_shaded_cull_back,
                        (DrawPhase::Opaque, Some(wgpu::Face::Front)) => self.rp_shaded_cull_front,
                        (DrawPhase::PickingLayer, None) => self.rp_picking_layer,
                        (DrawPhase::PickingLayer, Some(wgpu::Face::Back)) => {
                            self.rp_picking_layer_cull_back
                        }
                        (DrawPhase::PickingLayer, Some(wgpu::Face::Front)) => {
                            self.rp_picking_layer_cull_front
                        }
                        (DrawPhase::OutlineMask, None) => self.rp_outline_mask,
                        (DrawPhase::OutlineMask, Some(wgpu::Face::Back)) => {
                            self.rp_outline_mask_cull_back
                        }
                        (DrawPhase::OutlineMask, Some(wgpu::Face::Front)) => {
                            self.rp_outline_mask_cull_front
                        }
                        _ => unreachable!(),
                    };
                    pass.set_pipeline(render_pipelines.get(pipeline)?);
                }

                for material in &mesh_batch.mesh.materials {
                    if phase == DrawPhase::Transparent
                        && !material.has_transparency
                        && !mesh_batch.has_transparent_tint
                    {
                        // Skip if this material is to be handled by opaque drawables.
                        continue;
                    }
                    if phase == DrawPhase::Opaque && material.has_transparency {
                        // Skip if this is to be handled by transparent drawables.
                        continue;
                    }

                    pass.set_bind_group(1, &material.bind_group, &[]);

                    let indices = material.index_range.clone();
                    let instances = mesh_batch.instance_range.clone();
                    if phase == DrawPhase::Transparent {
                        match mesh_batch.cull_mode {
                            None => {
                                // Default two-pass: first cull front faces, then cull back faces.
                                pass.set_pipeline(
                                    render_pipelines
                                        .get(self.rp_shaded_alpha_blended_cull_front)?,
                                );
                                pass.draw_indexed(indices.clone(), 0, instances.clone());

                                pass.set_pipeline(
                                    render_pipelines.get(self.rp_shaded_alpha_blended_cull_back)?,
                                );
                                pass.draw_indexed(indices, 0, instances);
                            }
                            Some(wgpu::Face::Back) => {
                                pass.set_pipeline(
                                    render_pipelines.get(self.rp_shaded_alpha_blended_cull_back)?,
                                );
                                pass.draw_indexed(indices, 0, instances);
                            }
                            Some(wgpu::Face::Front) => {
                                pass.set_pipeline(
                                    render_pipelines
                                        .get(self.rp_shaded_alpha_blended_cull_front)?,
                                );
                                pass.draw_indexed(indices, 0, instances);
                            }
                        }
                    } else {
                        pass.draw_indexed(indices, 0, instances);
                    }
                }
            }
        }

        Ok(())
    }
}

/// Determines which draw phases an mesh instance participates in.
#[expect(clippy::fn_params_excessive_bools)] // private function 🤷‍♂️
fn instance_draw_phases(
    instance: &GpuMeshInstance,
    any_material_transparent: bool,
    all_materials_transparent: bool,
) -> EnumSet<DrawPhase> {
    let mut phases = EnumSet::from(DrawPhase::PickingLayer);

    if instance.outline_mask_ids.is_some() {
        phases.insert(DrawPhase::OutlineMask);
    }

    if !instance.additive_tint.is_opaque() {
        // Everything is transparently tinted.
        phases.insert(DrawPhase::Transparent);
    } else {
        if any_material_transparent {
            phases.insert(DrawPhase::Transparent);
        }
        if !all_materials_transparent {
            phases.insert(DrawPhase::Opaque);
        }
    }

    phases
}

#[cfg(test)]
mod tests {
    use smallvec::SmallVec;

    use super::*;
    use crate::mesh::{CpuMesh, GpuMesh, Material};
    use crate::{Color32, DrawPhaseManager, PickingLayerId, RenderContext, Rgba32Unmul};

    fn test_view_info() -> DrawableCollectionViewInfo {
        DrawableCollectionViewInfo {
            camera_world_position: glam::Vec3A::ZERO,
        }
    }

    fn test_mesh(ctx: &RenderContext, materials: SmallVec<[Material; 1]>) -> GpuMesh {
        let vertex_positions = vec![
            glam::Vec3::new(0.0, 1.0, 0.0),
            glam::Vec3::new(-1.0, -1.0, 0.0),
            glam::Vec3::new(1.0, -1.0, 0.0),
        ];
        let bbox = crate::util::bounding_box_from_points(vertex_positions.iter().copied());
        let cpu_mesh = CpuMesh {
            label: "test_mesh".into(),
            triangle_indices: vec![glam::UVec3::new(0, 1, 2)],
            vertex_positions,
            vertex_colors: vec![Rgba32Unmul::WHITE; 3],
            vertex_normals: vec![glam::Vec3::new(0.0, 0.0, 1.0); 3],
            vertex_texcoords: vec![glam::Vec2::ZERO; 3],
            materials,
            bbox,
        };

        GpuMesh::new(ctx, &cpu_mesh).unwrap()
    }

    fn opaque_test_mesh(ctx: &RenderContext) -> GpuMesh {
        test_mesh(
            ctx,
            smallvec![Material {
                label: "opaque_material".into(),
                index_range: 0..3,
                albedo: ctx.texture_manager_2d.white_texture_unorm_handle().clone(),
                albedo_factor: crate::Rgba::WHITE
            }],
        )
    }

    fn opaque_and_transparent_test_mesh(ctx: &RenderContext) -> GpuMesh {
        test_mesh(
            ctx,
            smallvec![
                Material {
                    label: "opaque_material".into(),
                    index_range: 0..3,
                    albedo: ctx.texture_manager_2d.white_texture_unorm_handle().clone(),
                    albedo_factor: crate::Rgba::WHITE
                },
                Material {
                    label: "opaque_material".into(),
                    index_range: 0..3,
                    albedo: ctx.texture_manager_2d.white_texture_unorm_handle().clone(),
                    albedo_factor: crate::Rgba::TRANSPARENT
                }
            ],
        )
    }

    fn mesh_instance(gpu_mesh: Arc<GpuMesh>) -> GpuMeshInstance {
        GpuMeshInstance {
            gpu_mesh,
            world_from_mesh: glam::Affine3A::IDENTITY,
            additive_tint: Color32::WHITE,
            outline_mask_ids: OutlineMaskPreference::NONE,
            picking_layer_id: PickingLayerId::default(),
            cull_mode: None,
        }
    }

    #[test]
    fn test_simple_opaque() {
        let ctx = RenderContext::new_test();
        let mesh = Arc::new(opaque_test_mesh(&ctx));

        let instance_no_tint_no_outline = mesh_instance(mesh.clone());
        let instances = vec![
            instance_no_tint_no_outline.clone(),
            instance_no_tint_no_outline.clone(),
        ];

        // This should create one bach each for the two active layers (picking & opaque).
        let draw_data = MeshDrawData::new(&ctx, &instances).unwrap();
        assert_eq!(draw_data.batches.len(), 2);
        assert_eq!(draw_data.batches[0].instance_range.len(), 2);
        assert_eq!(draw_data.batches[0].draw_phase, DrawPhase::Opaque);
        assert_eq!(draw_data.batches[1].instance_range.len(), 2);
        assert_eq!(draw_data.batches[1].draw_phase, DrawPhase::PickingLayer);

        let mut draw_phase_manager = DrawPhaseManager::new(EnumSet::all());
        draw_phase_manager.add_draw_data(&ctx, draw_data.into(), &test_view_info());

        let opaque_drawables = draw_phase_manager.drawables_for_phase(DrawPhase::Opaque);
        assert_eq!(opaque_drawables.len(), 1);
        assert_eq!(opaque_drawables[0].draw_data_payload, 0);

        let picking_drawables = draw_phase_manager.drawables_for_phase(DrawPhase::PickingLayer);
        assert_eq!(picking_drawables.len(), 1);
        assert_eq!(picking_drawables[0].draw_data_payload, 1);
    }

    #[test]
    fn test_transparent_tint() {
        let ctx = RenderContext::new_test();
        let mesh = Arc::new(opaque_test_mesh(&ctx));

        // Middle meshes have transparent tint, rest not.
        let instance_no_tint_no_outline = mesh_instance(mesh.clone());
        let instance_transparent_tint_no_outline = GpuMeshInstance {
            additive_tint: Color32::TRANSPARENT,
            ..instance_no_tint_no_outline.clone()
        };
        let instances = vec![
            instance_no_tint_no_outline.clone(),
            instance_transparent_tint_no_outline.clone(),
            instance_transparent_tint_no_outline.clone(),
            instance_no_tint_no_outline.clone(),
        ];

        // This should still create only one batch for picking & opaque,
        // but two additional ones for the ones with transparent tint (these never batch).
        let draw_data = MeshDrawData::new(&ctx, &instances).unwrap();
        assert_eq!(draw_data.batches.len(), 4);
        assert_eq!(draw_data.batches[0].instance_range.len(), 1);
        assert_eq!(draw_data.batches[0].draw_phase, DrawPhase::Transparent);
        assert!(draw_data.batches[1].has_transparent_tint);
        assert_eq!(draw_data.batches[1].instance_range.len(), 1);
        assert_eq!(draw_data.batches[1].draw_phase, DrawPhase::Transparent);
        assert!(draw_data.batches[1].has_transparent_tint);
        assert_eq!(draw_data.batches[2].instance_range.len(), 2);
        assert_eq!(draw_data.batches[2].draw_phase, DrawPhase::Opaque);
        assert_eq!(draw_data.batches[3].instance_range.len(), 4);
        assert_eq!(draw_data.batches[3].draw_phase, DrawPhase::PickingLayer);

        let mut draw_phase_manager = DrawPhaseManager::new(EnumSet::all());
        draw_phase_manager.add_draw_data(&ctx, draw_data.into(), &test_view_info());

        let opaque_drawables = draw_phase_manager.drawables_for_phase(DrawPhase::Opaque);
        assert_eq!(opaque_drawables.len(), 1);
        assert_eq!(opaque_drawables[0].draw_data_payload, 2);

        let transparent_drawables = draw_phase_manager.drawables_for_phase(DrawPhase::Transparent);
        assert_eq!(transparent_drawables.len(), 2);
        assert_eq!(transparent_drawables[0].draw_data_payload, 0);
        assert_eq!(transparent_drawables[1].draw_data_payload, 1);

        let picking_drawables = draw_phase_manager.drawables_for_phase(DrawPhase::PickingLayer);
        assert_eq!(picking_drawables.len(), 1);
        assert_eq!(picking_drawables[0].draw_data_payload, 3);
    }

    #[test]
    fn test_outlines() {
        let ctx = RenderContext::new_test();
        let mesh = Arc::new(opaque_test_mesh(&ctx));

        // Some meshes have outlines, some don't.
        let instance_no_tint_no_outline = mesh_instance(mesh.clone());
        let instance_no_tint_outlines = GpuMeshInstance {
            outline_mask_ids: OutlineMaskPreference::some(1, 2),
            ..instance_no_tint_no_outline.clone()
        };
        let instances = vec![
            instance_no_tint_outlines.clone(),
            instance_no_tint_no_outline.clone(),
            instance_no_tint_outlines.clone(),
            instance_no_tint_no_outline.clone(),
        ];

        // This should still create only one batch for picking & opaque,
        // but additional outline for the instance with outlines..
        let draw_data = MeshDrawData::new(&ctx, &instances).unwrap();
        assert_eq!(draw_data.batches.len(), 3);
        assert_eq!(draw_data.batches[0].instance_range.len(), 4); // All draw outlines.
        assert_eq!(draw_data.batches[0].draw_phase, DrawPhase::Opaque);
        assert_eq!(draw_data.batches[1].instance_range.len(), 2); // Two outlines, batched together.
        assert_eq!(draw_data.batches[1].draw_phase, DrawPhase::OutlineMask);
        assert_eq!(draw_data.batches[2].instance_range.len(), 4); // All draw picking.
        assert_eq!(draw_data.batches[2].draw_phase, DrawPhase::PickingLayer);

        let mut draw_phase_manager = DrawPhaseManager::new(EnumSet::all());
        draw_phase_manager.add_draw_data(&ctx, draw_data.into(), &test_view_info());

        let opaque_drawables = draw_phase_manager.drawables_for_phase(DrawPhase::Opaque);
        assert_eq!(opaque_drawables.len(), 1);
        assert_eq!(opaque_drawables[0].draw_data_payload, 0);

        let outline_drawables = draw_phase_manager.drawables_for_phase(DrawPhase::OutlineMask);
        assert_eq!(outline_drawables.len(), 1);
        assert_eq!(outline_drawables[0].draw_data_payload, 1);

        let picking_drawables = draw_phase_manager.drawables_for_phase(DrawPhase::PickingLayer);
        assert_eq!(picking_drawables.len(), 1);
        assert_eq!(picking_drawables[0].draw_data_payload, 2);
    }

    #[test]
    fn test_opaque_and_transparent_materials() {
        let ctx = RenderContext::new_test();
        let mesh = Arc::new(opaque_and_transparent_test_mesh(&ctx));

        // Each instance has both an opaque and a transparent material.
        let instance_no_tint_no_outline = mesh_instance(mesh.clone());
        let instances = vec![
            instance_no_tint_no_outline.clone(),
            instance_no_tint_no_outline.clone(),
        ];

        // Transparent instances can't be batched!
        let draw_data = MeshDrawData::new(&ctx, &instances).unwrap();
        assert_eq!(draw_data.batches.len(), 4);
        assert_eq!(draw_data.batches[0].instance_range.len(), 1);
        assert_eq!(draw_data.batches[0].draw_phase, DrawPhase::Transparent);
        assert_eq!(draw_data.batches[1].instance_range.len(), 1);
        assert_eq!(draw_data.batches[1].draw_phase, DrawPhase::Transparent);
        assert_eq!(draw_data.batches[2].instance_range.len(), 2);
        assert_eq!(draw_data.batches[2].draw_phase, DrawPhase::Opaque);
        assert_eq!(draw_data.batches[3].instance_range.len(), 2);
        assert_eq!(draw_data.batches[3].draw_phase, DrawPhase::PickingLayer);

        let mut draw_phase_manager = DrawPhaseManager::new(EnumSet::all());
        draw_phase_manager.add_draw_data(&ctx, draw_data.into(), &test_view_info());

        let opaque_drawables = draw_phase_manager.drawables_for_phase(DrawPhase::Opaque);
        assert_eq!(opaque_drawables.len(), 1);
        assert_eq!(opaque_drawables[0].draw_data_payload, 2);

        let transparent_drawables = draw_phase_manager.drawables_for_phase(DrawPhase::Transparent);
        assert_eq!(transparent_drawables.len(), 2);
        assert_eq!(transparent_drawables[0].draw_data_payload, 0);
        assert_eq!(transparent_drawables[1].draw_data_payload, 1);

        let picking_drawables = draw_phase_manager.drawables_for_phase(DrawPhase::PickingLayer);
        assert_eq!(picking_drawables.len(), 1);
        assert_eq!(picking_drawables[0].draw_data_payload, 3);
    }
}
