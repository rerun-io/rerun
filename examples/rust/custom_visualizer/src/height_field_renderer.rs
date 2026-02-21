use std::sync::Arc;

use rerun::external::glam;
use rerun::external::re_renderer::external::smallvec::smallvec;
use rerun::external::re_renderer::external::wgpu;
use rerun::external::re_renderer::{self};

mod gpu_data {
    use rerun::external::re_renderer::{self, wgpu_buffer_types};

    /// Keep in sync with `UniformBuffer` in `height_field.wgsl`.
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct UniformBuffer {
        pub world_from_obj: wgpu_buffer_types::Mat4,

        pub grid_cols: u32,
        pub grid_rows: u32,
        pub spacing: f32,
        pub colormap: u32,

        pub min_height: f32,
        pub max_height: f32,
        pub _pad0: f32,
        pub _pad1: f32,

        pub picking_layer_object_id: re_renderer::PickingLayerObjectId,
        pub picking_instance_id: re_renderer::PickingLayerInstanceId,

        pub outline_mask: wgpu_buffer_types::UVec2RowPadded,

        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 8],
    }
}

/// Implements a custom [`re_renderer::renderer::Renderer`] for drawing heightfield meshes.
///
/// The vertex shader receives a single `f32` height per vertex via a vertex buffer,
/// derives grid row/col from `@builtin(vertex_index)` (fed by the index buffer),
/// and computes positions and colormap colors. Normals are derived in the fragment
/// shader from screen-space derivatives of the interpolated world position.
pub struct HeightFieldRenderer {
    bind_group_layout: re_renderer::GpuBindGroupLayoutHandle,

    render_pipeline_color: re_renderer::GpuRenderPipelineHandle,
    render_pipeline_picking_layer: re_renderer::GpuRenderPipelineHandle,
    render_pipeline_outline_mask: re_renderer::GpuRenderPipelineHandle,
}

/// Properties describing a single heightfield mesh to be rendered.
pub struct HeightFieldConfig<'a> {
    /// Transform from object/local space to world space.
    pub world_from_obj: glam::Affine3A,

    /// Flat, row-major array of height values (`grid_rows * grid_cols` elements).
    pub heights: &'a [f32],

    /// Number of columns in the height grid.
    pub grid_cols: u32,

    /// Number of rows in the height grid.
    pub grid_rows: u32,

    /// World-space distance between adjacent grid points.
    pub spacing: f32,

    /// Minimum height value (used for colormap normalization).
    pub min_height: f32,

    /// Maximum height value (used for colormap normalization).
    pub max_height: f32,

    /// Colormap enum value (as `u32`) passed to the shader.
    pub colormap: u32,

    /// Picking layer object ID for hit-testing.
    pub picking_layer_object_id: re_renderer::PickingLayerObjectId,

    /// Picking layer instance ID for hit-testing.
    pub picking_instance_id: re_renderer::PickingLayerInstanceId,

    /// Outline mask for selection highlighting.
    pub outline_mask: re_renderer::OutlineMaskPreference,
}

/// GPU draw data for drawing heightfield meshes using [`HeightFieldRenderer`].
#[derive(Clone)]
pub struct HeightFieldDrawData {
    meshes: Vec<MeshInstance>,
}

#[derive(Clone)]
struct MeshInstance {
    bind_group: re_renderer::GpuBindGroup,
    vertex_buffer: Arc<wgpu::Buffer>,
    index_buffer: Arc<wgpu::Buffer>,
    num_indices: u32,
    has_outline: bool,
}

impl re_renderer::renderer::DrawData for HeightFieldDrawData {
    type Renderer = HeightFieldRenderer;

    fn collect_drawables(
        &self,
        _view_info: &re_renderer::renderer::DrawableCollectionViewInfo,
        collector: &mut re_renderer::DrawableCollector<'_>,
    ) {
        use re_renderer::renderer::DrawDataDrawable;

        for (i, mesh) in self.meshes.iter().enumerate() {
            collector.add_drawable(
                re_renderer::DrawPhase::Opaque | re_renderer::DrawPhase::PickingLayer,
                DrawDataDrawable {
                    distance_sort_key: 0.0,
                    draw_data_payload: i as u32,
                },
            );

            if mesh.has_outline {
                collector.add_drawable(
                    re_renderer::DrawPhase::OutlineMask,
                    DrawDataDrawable {
                        distance_sort_key: 0.0,
                        draw_data_payload: i as u32,
                    },
                );
            }
        }
    }
}

impl HeightFieldDrawData {
    pub fn new(ctx: &re_renderer::RenderContext) -> Self {
        let _ = ctx.renderer::<HeightFieldRenderer>();
        Self { meshes: Vec::new() }
    }

    /// Adds a heightfield mesh to the draw data.
    ///
    /// Heights are uploaded as a vertex buffer with a single `f32` attribute per grid vertex.
    pub fn add_mesh(
        &mut self,
        ctx: &re_renderer::RenderContext,
        label: &str,
        config: &HeightFieldConfig<'_>,
    ) {
        if config.grid_cols < 2 || config.grid_rows < 2 {
            return;
        }

        let renderer = ctx.renderer::<HeightFieldRenderer>();

        // --- Uniform buffer (via re_renderer helper) ----------------------------

        let uniform = gpu_data::UniformBuffer {
            world_from_obj: config.world_from_obj.into(),
            grid_cols: config.grid_cols,
            grid_rows: config.grid_rows,
            spacing: config.spacing,
            colormap: config.colormap,
            min_height: config.min_height,
            max_height: config.max_height,
            _pad0: 0.0,
            _pad1: 0.0,
            picking_layer_object_id: config.picking_layer_object_id,
            picking_instance_id: config.picking_instance_id,
            outline_mask: config.outline_mask.0.unwrap_or_default().into(),
            end_padding: Default::default(),
        };
        let uniform_buffer_entry =
            re_renderer::create_and_fill_uniform_buffer(ctx, label.into(), uniform);

        // --- Heights vertex buffer (one f32 per grid vertex) --------------------

        let vertex_data: &[u8] = bytemuck::cast_slice(config.heights);
        let vertex_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{label}::height_vertex_buffer")),
            size: vertex_data.len() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: true,
        });
        vertex_buffer
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(vertex_data);
        vertex_buffer.unmap();

        // --- Bind group (uniform buffer only) ----------------------------------

        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &re_renderer::BindGroupDesc {
                label: label.into(),
                entries: smallvec![uniform_buffer_entry],
                layout: renderer.bind_group_layout,
            },
        );

        // Build an index buffer so the GPU vertex cache can reuse shared vertices.
        // Each quad is two triangles: tl->bl->tr, tr->bl->br.
        let num_indices = (config.grid_rows - 1) * (config.grid_cols - 1) * 6;
        let mut indices: Vec<u32> = Vec::with_capacity(num_indices as usize);
        for row in 0..config.grid_rows - 1 {
            for col in 0..config.grid_cols - 1 {
                let tl = row * config.grid_cols + col;
                let tr = tl + 1;
                let bl = tl + config.grid_cols;
                let br = bl + 1;
                indices.push(tl);
                indices.push(bl);
                indices.push(tr);
                indices.push(tr);
                indices.push(bl);
                indices.push(br);
            }
        }

        let index_data: &[u8] = bytemuck::cast_slice(&indices);
        let index_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("heightfield_index_buffer"),
            size: index_data.len() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::INDEX,
            mapped_at_creation: true,
        });
        index_buffer
            .slice(..)
            .get_mapped_range_mut()
            .copy_from_slice(index_data);
        index_buffer.unmap();

        self.meshes.push(MeshInstance {
            bind_group,
            vertex_buffer: Arc::new(vertex_buffer),
            index_buffer: Arc::new(index_buffer),
            num_indices,
            has_outline: config.outline_mask.is_some(),
        });
    }
}

impl re_renderer::renderer::Renderer for HeightFieldRenderer {
    type RendererDrawData = HeightFieldDrawData;

    fn create_renderer(ctx: &re_renderer::RenderContext) -> Self {
        let shader_modules = &ctx.gpu_resources.shader_modules;
        let shader_module = shader_modules.get_or_create(
            ctx,
            &re_renderer::include_shader_module!("../shader/height_field.wgsl"),
        );

        // Bind group layout: uniform buffer only (heights come via vertex buffer).
        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &re_renderer::BindGroupLayoutDesc {
                label: "HeightFieldRenderer::bind_group_layout".into(),
                entries: vec![wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                            gpu_data::UniformBuffer,
                        >()
                            as _),
                    },
                    count: None,
                }],
            },
        );

        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &re_renderer::PipelineLayoutDesc {
                label: "HeightFieldRenderer".into(),
                entries: vec![ctx.global_bindings.layout, bind_group_layout],
            },
        );

        // Single vertex buffer: one f32 height per grid vertex.
        let render_pipeline_desc_color = re_renderer::RenderPipelineDesc {
            label: "HeightFieldRenderer::color".into(),
            pipeline_layout,
            vertex_entrypoint: "vs_main".into(),
            vertex_handle: shader_module,
            fragment_entrypoint: "fs_main".into(),
            fragment_handle: shader_module,
            vertex_buffers: re_renderer::VertexBufferLayout::from_formats(
                [wgpu::VertexFormat::Float32].into_iter(),
            ),
            render_targets: smallvec![Some(
                re_renderer::ViewBuilder::MAIN_TARGET_COLOR_FORMAT.into()
            )],
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(re_renderer::ViewBuilder::MAIN_TARGET_DEFAULT_DEPTH_STATE),
            multisample: re_renderer::ViewBuilder::main_target_default_msaa_state(
                ctx.render_config(),
                false,
            ),
        };

        let render_pipelines = &ctx.gpu_resources.render_pipelines;
        let render_pipeline_color =
            render_pipelines.get_or_create(ctx, &render_pipeline_desc_color);
        let render_pipeline_picking_layer = render_pipelines.get_or_create(
            ctx,
            &re_renderer::RenderPipelineDesc {
                label: "HeightFieldRenderer::picking_layer".into(),
                fragment_entrypoint: "fs_main_picking_layer".into(),
                render_targets: smallvec![Some(
                    re_renderer::PickingLayerProcessor::PICKING_LAYER_FORMAT.into()
                )],
                depth_stencil: re_renderer::PickingLayerProcessor::PICKING_LAYER_DEPTH_STATE,
                multisample: re_renderer::PickingLayerProcessor::PICKING_LAYER_MSAA_STATE,
                ..render_pipeline_desc_color.clone()
            },
        );
        let render_pipeline_outline_mask = render_pipelines.get_or_create(
            ctx,
            &re_renderer::RenderPipelineDesc {
                label: "HeightFieldRenderer::outline_mask".into(),
                fragment_entrypoint: "fs_main_outline_mask".into(),
                render_targets: smallvec![Some(
                    re_renderer::OutlineMaskProcessor::MASK_FORMAT.into()
                )],
                depth_stencil: re_renderer::OutlineMaskProcessor::MASK_DEPTH_STATE,
                ..render_pipeline_desc_color
            },
        );

        Self {
            bind_group_layout,
            render_pipeline_color,
            render_pipeline_outline_mask,
            render_pipeline_picking_layer,
        }
    }

    fn draw(
        &self,
        render_pipelines: &re_renderer::GpuRenderPipelinePoolAccessor<'_>,
        phase: re_renderer::DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_instructions: &[re_renderer::renderer::DrawInstruction<'_, HeightFieldDrawData>],
    ) -> Result<(), re_renderer::renderer::DrawError> {
        let pipeline_handle = match phase {
            re_renderer::DrawPhase::Opaque => self.render_pipeline_color,
            re_renderer::DrawPhase::OutlineMask => self.render_pipeline_outline_mask,
            re_renderer::DrawPhase::PickingLayer => self.render_pipeline_picking_layer,
            _ => unreachable!("We were called on a phase we weren't subscribed to: {phase:?}"),
        };

        let pipeline = render_pipelines.get(pipeline_handle)?;
        pass.set_pipeline(pipeline);

        for instruction in draw_instructions {
            for drawable in instruction.drawables {
                let mesh_index = drawable.draw_data_payload as usize;
                if let Some(mesh) = instruction.draw_data.meshes.get(mesh_index) {
                    if phase == re_renderer::DrawPhase::OutlineMask && !mesh.has_outline {
                        continue;
                    }

                    pass.set_bind_group(1, &*mesh.bind_group, &[]);
                    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.num_indices, 0, 0..1);
                }
            }
        }

        Ok(())
    }
}
