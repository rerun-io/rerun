use crate::texture_pool::TexturePool;

/// Any resource involving wgpu rendering which can be re-used accross different scenes.
/// I.e. render pipelines, resource pools, etc.
pub struct RenderContext {
    /// The color format used by the eframe output buffer.
    output_format_color: wgpu::TextureFormat,

    /// The depth format used by the eframe output buffer.
    /// TODO(andreas): Should we maintain depth buffers per view and ask for no depth from eframe?
    output_format_depth: Option<wgpu::TextureFormat>,

    // TODO(andreas): Introduce a pipeline manager
    test_triangle: Option<wgpu::RenderPipeline>,

    // TODO(andreas): Establish a trait for pools to give them a similar interface and allow iterating over them etc.
    pub(crate) texture_pool: TexturePool,

    frame_index: u64,
}

/// Render pipeline handle that needs to be requested from the `RenderContext` and can be resolved to a `wgpu::RenderPipeline` before drawing.
#[derive(Clone, Copy)]
pub(crate) struct RenderPipelineHandle;

impl RenderContext {
    pub fn new(
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        output_format_color: wgpu::TextureFormat,
        output_format_depth: Option<wgpu::TextureFormat>,
    ) -> Self {
        RenderContext {
            output_format_color,
            output_format_depth,
            test_triangle: None,
            texture_pool: TexturePool::new(),
            frame_index: 0,
        }
    }

    /// Requests a render pipeline and returns a handle to it.
    ///
    /// Internally, this ensures the requested pipeline is created and tracked.
    /// Returns a handle even if creating the pipeline fails!
    /// (this might be due to shader compilation error that might be fixed later)
    pub(crate) fn request_render_pipeline(
        &mut self,
        device: &wgpu::Device,
    ) -> RenderPipelineHandle {
        self.test_triangle.get_or_insert_with(|| {
            // TODO(andreas): Standardize bind group and render pipeline layouts so we only ever have a handful.
            // (is this feasable?)
            // let bind_group_layout =
            //     device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            //         label: Some("custom3d"),
            //         entries: &[],
            //     });
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("custom3d"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("custom3d"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("../shader/test_triangle.wgsl").into(),
                ),
            });
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("test triangle"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(self.output_format_color.into())],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: self
                    .output_format_depth
                    .map(|format| wgpu::DepthStencilState {
                        format,
                        depth_compare: wgpu::CompareFunction::Always,
                        depth_write_enabled: false,
                        stencil: Default::default(),
                        bias: Default::default(),
                    }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            })
        });

        RenderPipelineHandle
    }

    /// Retrieves a [`wgpu::RenderPipeline`] given a handle.
    /// Returns None if the pipeline does not exist or failed to create.
    pub(crate) fn render_pipeline(
        &self,
        _handle: RenderPipelineHandle,
    ) -> Option<&wgpu::RenderPipeline> {
        // TODO(andreas)render_context
        self.test_triangle.as_ref()
    }

    pub fn frame_maintenance(&mut self) {
        self.frame_index += 1;
        self.texture_pool.frame_maintenance(self.frame_index);
    }
}
