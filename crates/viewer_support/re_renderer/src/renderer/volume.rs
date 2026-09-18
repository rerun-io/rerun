use enumset::{EnumSet, enum_set};
use smallvec::smallvec;

use super::{DrawData, DrawError, RenderContext, Renderer};
use crate::allocator::create_and_fill_uniform_buffer;
use crate::draw_phases::{DrawPhase, OutlineMaskProcessor, PickingLayerProcessor};
use crate::renderer::{DrawDataDrawable, DrawInstruction, DrawableCollectionViewInfo};
use crate::resource_managers::GpuTexture3D;
use crate::wgpu_resources::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, BufferDesc, GpuBindGroup,
    GpuBindGroupLayoutHandle, GpuBuffer, GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor,
    PipelineLayoutDesc, RenderPipelineDesc, VertexBufferLayout,
};
use crate::{
    Colormap, DrawableCollector, OutlineMaskPreference, PickingLayerId, ViewBuilder,
    include_shader_module,
};

const CUBE_VERTICES: [[f32; 3]; 8] = [
    [0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [1.0, 1.0, 0.0],
    [0.0, 0.0, 1.0],
    [1.0, 0.0, 1.0],
    [0.0, 1.0, 1.0],
    [1.0, 1.0, 1.0],
];

const CUBE_TRIANGLES: [[usize; 3]; 12] = [
    [1, 3, 7],
    [1, 7, 5],
    [0, 4, 6],
    [0, 6, 2],
    [2, 6, 7],
    [2, 7, 3],
    [0, 1, 5],
    [0, 5, 4],
    [4, 5, 7],
    [4, 7, 6],
    [0, 2, 3],
    [0, 3, 1],
];

const VERTICES_PER_CUBE: usize = CUBE_TRIANGLES.len() * 3;

/// Rendering options for one dense scalar volume.
#[derive(Clone, Copy, Debug)]
pub struct VolumeOptions {
    /// Transform from normalized volume coordinates to world coordinates.
    pub world_from_volume: glam::Affine3A,

    /// Inclusive range of voxel values to render, in the raw units of the texture data.
    pub value_range: [f32; 2],

    /// Colormap applied to normalized scalar values.
    pub colormap: Colormap,

    /// Positive exponent applied to normalized density before colormapping and extinction.
    pub gamma: f32,

    /// Dimensionless optical depth over one volume-local axis through fully dense material.
    pub optical_density: f32,

    /// Picking identifier shared by the volume.
    pub picking_layer_id: PickingLayerId,

    /// Optional outline mask ids shared by the volume.
    pub outline_mask_ids: OutlineMaskPreference,
}

mod gpu_data {
    use crate::wgpu_buffer_types;

    /// Keep in sync with `volume.wgsl`.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct UniformBuffer {
        pub world_from_volume: wgpu_buffer_types::Mat4,
        pub volume_from_world: wgpu_buffer_types::Mat4,

        pub value_range: wgpu_buffer_types::Vec2,
        pub optical_density: f32,
        pub colormap: u32,

        pub outline_mask_ids: wgpu_buffer_types::UVec2,
        pub gamma: f32,
        pub reserved: u32,

        pub picking_layer_id: [u32; 4],

        pub end_padding: [wgpu_buffer_types::PaddingRow; 5],
    }
}

#[derive(Clone)]
/// GPU draw data for one raymarched volume.
pub struct VolumeDrawData {
    bind_group: GpuBindGroup,
    draw_order_position: glam::Vec3A,
    active_phases: EnumSet<DrawPhase>,
    reverse_winding: bool,
}

impl VolumeDrawData {
    /// Creates draw data for a 3D texture and its visualization options.
    pub fn new(
        ctx: &RenderContext,
        texture: &GpuTexture3D,
        options: VolumeOptions,
    ) -> Result<Self, crate::RendererRegistrationError> {
        let renderer = ctx.renderer::<VolumeRenderer>()?;
        let VolumeOptions {
            world_from_volume,
            value_range,
            colormap,
            gamma,
            optical_density,
            outline_mask_ids,
            picking_layer_id,
        } = options;

        let uniform_buffer = gpu_data::UniformBuffer {
            world_from_volume: glam::Mat4::from(world_from_volume).into(),
            volume_from_world: glam::Mat4::from(world_from_volume.inverse()).into(),
            value_range: value_range.into(),
            optical_density,
            colormap: colormap as u32,
            outline_mask_ids: outline_mask_ids.0.unwrap_or_default().into(),
            gamma,
            reserved: 0,
            picking_layer_id: picking_layer_id.into(),
            end_padding: Default::default(),
        };

        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label: "VolumeDrawData::bind_group".into(),
                entries: smallvec![
                    create_and_fill_uniform_buffer(
                        ctx,
                        "VolumeDrawData::uniform_buffer".into(),
                        uniform_buffer,
                    ),
                    BindGroupEntry::DefaultTextureView(texture.handle()),
                ],
                layout: renderer.bind_group_layout,
            },
        );

        let mut active_phases = enum_set![DrawPhase::Volume | DrawPhase::PickingLayer];
        if outline_mask_ids.is_some() {
            active_phases.insert(DrawPhase::OutlineMask);
        }

        Ok(Self {
            bind_group,
            draw_order_position: world_from_volume.transform_point3a(glam::Vec3A::splat(0.5)),
            active_phases,
            reverse_winding: world_from_volume.matrix3.determinant() < 0.0,
        })
    }
}

impl DrawData for VolumeDrawData {
    type Renderer = VolumeRenderer;

    fn collect_drawables(
        &self,
        view_info: &DrawableCollectionViewInfo,
        collector: &mut DrawableCollector<'_>,
    ) {
        collector.add_drawable(
            self.active_phases,
            DrawDataDrawable::from_world_position(view_info, self.draw_order_position, 0),
        );
    }
}

/// Draws a cube and raymarches its scalar 3D texture in the fragment shader.
pub struct VolumeRenderer {
    // Raymarching must draw with enabled culling, otherwise we raymarch twice!
    //
    // Negative-determinant transforms require reversed triangle winding to preserve exit faces.
    // Buffer 0 uses the original winding; buffer 1 reverses it.
    // Two tiny vertex buffers is much preferred to duplicating the render pipelines for this, since render pipelines are very heavy objects.
    cube_vertices: [GpuBuffer; 2],
    rp_volume: GpuRenderPipelineHandle,
    rp_outline_mask: GpuRenderPipelineHandle,
    rp_picking_layer: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for VolumeRenderer {
    type RendererDrawData = VolumeDrawData;

    fn create_renderer(ctx: &RenderContext) -> Self {
        re_tracing::profile_function!();

        let cube_vertices = [false, true].map(|reverse_winding| {
            let mut vertices: [[f32; 3]; VERTICES_PER_CUBE] =
                std::array::from_fn(|i| CUBE_VERTICES[CUBE_TRIANGLES[i / 3][i % 3]]);
            if reverse_winding {
                for triangle in vertices.chunks_exact_mut(3) {
                    triangle.swap(0, 2);
                }
            }
            let buffer = ctx.gpu_resources.buffers.alloc(
                &ctx.device,
                &BufferDesc {
                    label: format!(
                        "VolumeRenderer::cube_vertices reverse_winding={reverse_winding}"
                    )
                    .into(),
                    size: std::mem::size_of_val(&vertices) as u64,
                    usage: wgpu::BufferUsages::VERTEX,
                    mapped_at_creation: true,
                },
            );
            buffer
                .slice(..)
                .get_mapped_range_mut()
                .expect("new cube vertex buffer is mapped at creation")
                .copy_from_slice(bytemuck::cast_slice(&vertices));
            buffer.unmap();
            buffer
        });

        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "VolumeRenderer::bind_group_layout".into(),
                entries: vec![
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: (std::mem::size_of::<gpu_data::UniformBuffer>()
                                as u64)
                                .try_into()
                                .ok(),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            },
        );
        let scene_depth_bind_group_layout = ViewBuilder::scene_depth_bind_group_layout(ctx);
        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &PipelineLayoutDesc {
                label: "VolumeRenderer::pipeline_layout".into(),
                entries: vec![
                    ctx.global_bindings.layout,
                    scene_depth_bind_group_layout,
                    bind_group_layout,
                ],
            },
        );
        let shader_desc = include_shader_module!("../../shader/volume.wgsl");
        let shader_module = ctx
            .gpu_resources
            .shader_modules
            .get_or_create(ctx, &shader_desc);

        let base_desc = RenderPipelineDesc {
            label: "VolumeRenderer::rp_volume".into(),
            pipeline_layout,
            vertex_entrypoint: "vs_main".into(),
            vertex_handle: shader_module,
            fragment_entrypoint: "fs_main".into(),
            fragment_handle: shader_module,
            vertex_buffers: VertexBufferLayout::from_formats(
                [wgpu::VertexFormat::Float32x3].into_iter(),
            ),
            render_targets: smallvec![Some(wgpu::ColorTargetState {
                format: ViewBuilder::MAIN_TARGET_COLOR_FORMAT,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Front),
                ..Default::default()
            },
            depth_stencil: None,
            multisample: ViewBuilder::main_target_default_msaa_state(ctx.render_config(), false),
        };
        let rp_volume = ctx
            .gpu_resources
            .render_pipelines
            .get_or_create(ctx, &base_desc);
        let rp_picking_layer = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "VolumeRenderer::rp_picking_layer".into(),
                fragment_entrypoint: "fs_main_picking_layer".into(),
                render_targets: smallvec![Some(PickingLayerProcessor::PICKING_LAYER_FORMAT.into())],
                depth_stencil: PickingLayerProcessor::PICKING_LAYER_DEPTH_STATE,
                multisample: PickingLayerProcessor::PICKING_LAYER_MSAA_STATE,
                ..base_desc.clone()
            },
        );
        let rp_outline_mask = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "VolumeRenderer::rp_outline_mask".into(),
                fragment_entrypoint: "fs_main_outline_mask".into(),
                render_targets: smallvec![Some(OutlineMaskProcessor::MASK_FORMAT.into())],
                depth_stencil: OutlineMaskProcessor::MASK_DEPTH_STATE,
                multisample: OutlineMaskProcessor::mask_default_msaa_state(ctx.device_caps().tier),
                ..base_desc
            },
        );

        Self {
            cube_vertices,
            rp_volume,
            rp_outline_mask,
            rp_picking_layer,
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
        let pipeline = match phase {
            DrawPhase::Volume => self.rp_volume,
            DrawPhase::OutlineMask => self.rp_outline_mask,
            DrawPhase::PickingLayer => self.rp_picking_layer,
            _ => unreachable!("We were called on a phase we weren't subscribed to: {phase:?}"),
        };
        pass.set_pipeline(render_pipelines.get(pipeline)?);

        for DrawInstruction { draw_data, .. } in draw_instructions {
            pass.set_vertex_buffer(
                0,
                self.cube_vertices[usize::from(draw_data.reverse_winding)].slice(..),
            );
            pass.set_bind_group(2, &draw_data.bind_group, &[]);
            pass.draw(0..VERTICES_PER_CUBE as u32, 0..1);
        }

        Ok(())
    }
}
