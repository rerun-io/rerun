use anyhow::{Context, Ok};
use parking_lot::RwLock;
use std::sync::Arc;

use crate::{
    context::*,
    global_bindings::FrameUniformBuffer,
    renderer::{tonemapper::*, Drawable, Renderer},
    resource_pools::{
        bind_group_pool::GpuBindGroupHandleStrong, buffer_pool::BufferDesc, texture_pool::*,
    },
};

type DrawFn = dyn for<'a, 'b> Fn(&'b RenderContext, &'a mut wgpu::RenderPass<'b>) -> anyhow::Result<()>
    + Sync
    + Send;

struct QueuedDraw {
    draw_func: Box<DrawFn>,
    sorting_index: u32,
}

/// The highest level rendering block in `re_renderer`.
/// Used to build up/collect various resources and then send them off for rendering of  a single view.
#[derive(Default)]
pub struct ViewBuilder {
    /// Result of [`ViewBuilder::setup_view`] - needs to be `Option` sine some of the fields don't have a default.
    setup: Option<ViewTargetSetup>,
    queued_draws: Vec<QueuedDraw>, // &mut wgpu::RenderPass
}

struct ViewTargetSetup {
    tonemapping_drawable: TonemapperDrawable,

    bind_group_0: GpuBindGroupHandleStrong,
    hdr_render_target_msaa: GpuTextureHandleStrong,
    hdr_render_target_resolved: GpuTextureHandleStrong,
    depth_buffer: GpuTextureHandleStrong,

    resolution_in_pixel: [u32; 2],
    origin_in_pixel: [u32; 2],
}

/// [`ViewBuilder`] that can be shared between threads.
///
/// Innermost field is an Option, so it can be consumed for `composite`.
pub type SharedViewBuilder = Arc<RwLock<Option<ViewBuilder>>>;

/// Basic configuration for a target view.
#[derive(Debug, Clone)]
pub struct TargetConfiguration {
    pub resolution_in_pixel: [u32; 2],
    pub origin_in_pixel: [u32; 2],
    // TODO(cmc): other viewport stuff? scissor too? blend constant? stencil ref?
    pub view_from_world: macaw::IsoTransform,

    pub fov_y: f32,
    pub near_plane_distance: f32,

    /// Every target needs an individual as persistent as possible identifier.
    /// This is used to facilitate easier resource re-use.
    pub target_identifier: u64,
}

impl ViewBuilder {
    pub const MAIN_TARGET_COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
    pub const MAIN_TARGET_DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

    /// Enable MSAA always. This makes our pipeline less variable as well, as we need MSAA resolve steps if we want any MSAA at all!
    ///
    /// 4 samples are the only thing `WebGPU` supports, and currently wgpu as well
    /// ([tracking issue for more options on native](https://github.com/gfx-rs/wgpu/issues/2910))
    pub const MAIN_TARGET_SAMPLE_COUNT: u32 = 4;

    /// Default multisample state that any [`wgpu::RenderPipeline`] drawing to the main target needs to use.
    ///
    /// In rare cases, pipelines may want to enable alpha to coverage and/or sample masks.
    pub const MAIN_TARGET_DEFAULT_MSAA_STATE: wgpu::MultisampleState = wgpu::MultisampleState {
        count: ViewBuilder::MAIN_TARGET_SAMPLE_COUNT,
        mask: !0,
        alpha_to_coverage_enabled: false,
    };

    pub fn new_shared() -> SharedViewBuilder {
        Arc::new(RwLock::new(Some(ViewBuilder::default())))
    }

    pub fn setup_view(
        &mut self,
        ctx: &mut RenderContext,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &TargetConfiguration,
    ) -> anyhow::Result<&mut Self> {
        // TODO(andreas): Should tonemapping preferences go here as well? Likely!
        let hdr_render_target_desc = TextureDesc {
            label: "hdr rendertarget msaa".into(),
            size: wgpu::Extent3d {
                width: config.resolution_in_pixel[0],
                height: config.resolution_in_pixel[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: Self::MAIN_TARGET_SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format: Self::MAIN_TARGET_COLOR,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        };
        let hdr_render_target_msaa = ctx
            .resource_pools
            .textures
            .alloc(device, &hdr_render_target_desc);
        // Like hdr_render_target, but with MSAA resolved.
        let hdr_render_target_resolved = ctx.resource_pools.textures.alloc(
            device,
            &TextureDesc {
                label: "hdr rendertarget resolved".into(),
                sample_count: 1,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                ..hdr_render_target_desc
            },
        );
        let depth_buffer = ctx.resource_pools.textures.alloc(
            device,
            &TextureDesc {
                label: "depth buffer".into(),
                format: Self::MAIN_TARGET_DEPTH,
                ..hdr_render_target_desc
            },
        );

        let tonemapping_drawable =
            TonemapperDrawable::new(ctx, device, &hdr_render_target_resolved);

        // Setup frame uniform buffer
        let frame_uniform_buffer = ctx.resource_pools.buffers.alloc(
            device,
            &BufferDesc {
                label: "frame uniform buffer".into(),
                size: std::mem::size_of::<FrameUniformBuffer>() as _,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            },
        );

        let view_from_world = config.view_from_world.to_mat4();
        let camera_position = config.view_from_world.inverse().translation();

        // We use infinite reverse-z projection matrix.
        // * great precision both with floating point and integer: https://developer.nvidia.com/content/depth-precision-visualized
        // * no need to worry about far plane
        let projection_from_view = glam::Mat4::perspective_infinite_reverse_rh(
            config.fov_y,
            config.resolution_in_pixel[0] as f32 / config.resolution_in_pixel[1] as f32,
            config.near_plane_distance,
        );
        let projection_from_world = projection_from_view * view_from_world;

        let view_from_projection = projection_from_view.inverse();

        // Calculate the top right corner of the screen in view space.
        // Top right corner in projection space is (also called Normalized Device Coordinates) is (1, 1, 0)
        // (z zero means it sits on the near-plane)
        let top_right_screen_corner_in_view = view_from_projection
            .transform_point3(glam::vec3(1.0, 1.0, 0.0))
            .truncate()
            .normalize();

        queue.write_buffer(
            &ctx.resource_pools
                .buffers
                .get_resource(&frame_uniform_buffer)
                .unwrap()
                .buffer,
            0,
            bytemuck::bytes_of(&FrameUniformBuffer {
                view_from_world: glam::Affine3A::from_mat4(view_from_world).into(),
                projection_from_view: projection_from_view.into(),
                projection_from_world: projection_from_world.into(),
                camera_position: camera_position.into(),
                top_right_screen_corner_in_view: top_right_screen_corner_in_view.into(),
            }),
        );

        let bind_group_0 = ctx.shared_renderer_data.global_bindings.create_bind_group(
            &mut ctx.resource_pools,
            device,
            &frame_uniform_buffer,
        );

        self.setup = Some(ViewTargetSetup {
            tonemapping_drawable,
            bind_group_0,
            hdr_render_target_msaa,
            hdr_render_target_resolved,
            depth_buffer,
            resolution_in_pixel: config.resolution_in_pixel,
            origin_in_pixel: config.origin_in_pixel,
        });

        Ok(self)
    }

    pub fn queue_draw<D: Drawable + Sync + Send + Clone + 'static>(
        &mut self,
        draw_data: &D,
    ) -> &mut Self {
        let draw_data = draw_data.clone();

        self.queued_draws.push(QueuedDraw {
            draw_func: Box::new(move |ctx, pass| {
                let renderer = ctx
                    .renderers
                    .get::<D::Renderer>()
                    .context("failed to retrieve renderer")?;
                renderer.draw(&ctx.resource_pools, pass, &draw_data)
            }),
            sorting_index: D::Renderer::draw_order(),
        });

        self
    }

    /// Draws the frame as instructed to a temporary HDR target.
    pub fn draw(
        &mut self,
        ctx: &RenderContext,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        let setup = self
            .setup
            .as_ref()
            .context("ViewBuilder::setup_view wasn't called yet")?;

        let color_msaa = ctx
            .resource_pools
            .textures
            .get_resource(&setup.hdr_render_target_msaa)
            .context("hdr render target msaa")?;
        let color_resolved = ctx
            .resource_pools
            .textures
            .get_resource(&setup.hdr_render_target_resolved)
            .context("hdr render target resolved")?;
        let depth = ctx
            .resource_pools
            .textures
            .get_resource(&setup.depth_buffer)
            .context("depth buffer")?;

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("frame builder hdr pass"), // TODO(andreas): It would be nice to specify this from the outside so we know which view we're rendering
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &color_msaa.default_view,
                resolve_target: Some(&color_resolved.default_view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    // Don't care about the result, it's going to be resolved to the resolve target.
                    // This can have be much better perf, especially on tiler gpus.
                    store: false,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.default_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0), // 0.0 == far since we're using reverse-z
                    // Don't care about depth results afterwards.
                    // This can have be much better perf, especially on tiler gpus.
                    store: false,
                }),
                stencil_ops: None,
            }),
        });

        pass.set_bind_group(
            0,
            &ctx.resource_pools
                .bind_groups
                .get_resource(&setup.bind_group_0)
                .context("get global bind group")?
                .bind_group,
            &[],
        );

        self.queued_draws
            .sort_by(|a, b| a.sorting_index.cmp(&b.sorting_index));
        for queued_draw in &self.queued_draws {
            (queued_draw.draw_func)(ctx, &mut pass).context("drawing a view")?;
        }

        Ok(())
    }

    /// Applies tonemapping and draws the final result of a `ViewBuilder` to a given output `RenderPass`.
    ///
    /// The bound surface(s) on the `RenderPass` are expected to be the same format as specified on `Context` creation.
    pub fn composite<'a>(
        self,
        ctx: &'a RenderContext,
        pass: &mut wgpu::RenderPass<'a>,
    ) -> anyhow::Result<()> {
        let setup = self
            .setup
            .context("ViewBuilder::setup_view wasn't called yet")?;

        pass.set_viewport(
            setup.origin_in_pixel[0] as f32,
            setup.origin_in_pixel[1] as f32,
            setup.resolution_in_pixel[0] as f32,
            setup.resolution_in_pixel[1] as f32,
            0.0,
            1.0,
        );

        pass.set_bind_group(
            0,
            &ctx.resource_pools
                .bind_groups
                .get_resource(&setup.bind_group_0)
                .context("get global bind group")?
                .bind_group,
            &[],
        );

        let tonemapper = ctx
            .renderers
            .get::<Tonemapper>()
            .context("get tonemapper")?;
        tonemapper
            .draw(&ctx.resource_pools, pass, &setup.tonemapping_drawable)
            .context("perform tonemapping")
    }
}
