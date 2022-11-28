use anyhow::{Context, Ok};
use parking_lot::RwLock;
use std::sync::Arc;

use crate::{
    context::*,
    global_bindings::FrameUniformBuffer,
    renderer::{compositor::*, Drawable, Renderer},
    wgpu_resources::{BufferDesc, GpuBindGroupHandleStrong, GpuTextureHandleStrong, TextureDesc},
    DebugLabel,
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
    name: DebugLabel,

    tonemapping_drawable: CompositorDrawable,

    bind_group_0: GpuBindGroupHandleStrong,
    main_target_msaa: GpuTextureHandleStrong,
    main_target_resolved: GpuTextureHandleStrong,
    depth_buffer: GpuTextureHandleStrong,

    resolution_in_pixel: [u32; 2],
    origin_in_pixel: [u32; 2],
}

/// [`ViewBuilder`] that can be shared between threads.
///
/// Innermost field is an Option, so it can be consumed for `composite`.
pub type SharedViewBuilder = Arc<RwLock<Option<ViewBuilder>>>;

/// How we project from 3D to 2D.
#[derive(Debug, Clone)]
pub enum Projection {
    /// Perspective camera looking along the negative z view space axis.
    Perspective {
        /// Viewing angle in view space y direction (which is the vertical screen axis).
        vertical_fov: f32,

        /// Distance of the near plane. Everything behind is clipped.
        /// (we're looking into negative view-space z, but this is expected to be a positive value)
        near_plane_distance: f32,
    },

    /// Orthographic projection with the camera position at the near plane's center,
    /// looking along the negative z view space axis.
    ///
    /// Near plane is at z==0, everything with view space z>0 is clipped.
    Orthographic {
        /// Size of the orthographic camera view space y direction (which is the vertical screen axis).
        vertical_world_size: f32,

        /// Distance of the far plane to the camera
        /// (we're looking into negative view-space z, but this is expected to be a positive value)
        far_plane_distance: f32,
    },
}

/// Basic configuration for a target view.
#[derive(Debug, Clone)]
pub struct TargetConfiguration {
    pub name: DebugLabel,

    pub resolution_in_pixel: [u32; 2],
    pub origin_in_pixel: [u32; 2],

    pub view_from_world: macaw::IsoTransform,
    pub projection_from_view: Projection,
}

impl ViewBuilder {
    /// Color format used for the main target of the view builder.
    ///
    /// Eventually we'll want to make this an HDR format and apply tonemapping during composite.
    /// However, note that it is easy to run into subtle MSAA quality issues then:
    /// Applying MSAA resolve before tonemapping is problematic as it means we're doing msaa in linear.
    /// This is especially problematic at bright/dark edges where we may loose "smoothness"!
    /// For a nice illustration see [this blog post by MRP](https://therealmjp.github.io/posts/msaa-overview/)
    /// We either would need to keep the MSAA target and tonemap it,
    /// apply a manual resolve where we inverse-tonemap non-fully-covered pixel before averaging.
    /// (an optimized variant of this is described [by AMD here](https://gpuopen.com/learn/optimized-reversible-tonemapper-for-resolve/))
    /// In any case, this gets us onto a potentially much costlier rendering path, especially for tiling GPUs.
    pub const MAIN_TARGET_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

    /// Depth format used for the main target of the view builder.
    ///
    /// 32 bit float is widely supported, has best possible precision (with reverse infinite z projection)
    pub const MAIN_TARGET_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

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

    pub fn setup_view(
        &mut self,
        ctx: &mut RenderContext,
        config: TargetConfiguration,
    ) -> anyhow::Result<&mut Self> {
        crate::profile_function!();

        // Can't handle 0 size resolution since this would imply creating zero sized textures.
        assert_ne!(config.resolution_in_pixel[0], 0);
        assert_ne!(config.resolution_in_pixel[1], 0);

        // TODO(andreas): Should tonemapping preferences go here as well? Likely!
        let main_target_desc = TextureDesc {
            label: format!("{:?} - main target", config.name).into(),
            size: wgpu::Extent3d {
                width: config.resolution_in_pixel[0],
                height: config.resolution_in_pixel[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: Self::MAIN_TARGET_SAMPLE_COUNT,
            dimension: wgpu::TextureDimension::D2,
            format: Self::MAIN_TARGET_COLOR_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        };
        let hdr_render_target_msaa = ctx
            .resource_pools
            .textures
            .alloc(&ctx.device, &main_target_desc);
        // Like hdr_render_target, but with MSAA resolved.
        let main_target_resolved = ctx.resource_pools.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: format!("{:?} - main target resolved", config.name).into(),
                sample_count: 1,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                ..main_target_desc
            },
        );
        let depth_buffer = ctx.resource_pools.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: format!("{:?} - depth buffer", config.name).into(),
                format: Self::MAIN_TARGET_DEPTH_FORMAT,
                ..main_target_desc
            },
        );

        let tonemapping_drawable = CompositorDrawable::new(ctx, &main_target_resolved);

        // Setup frame uniform buffer
        let frame_uniform_buffer = ctx.resource_pools.buffers.alloc(
            &ctx.device,
            &BufferDesc {
                label: format!("{:?} - frame uniform buffer", config.name).into(),
                size: std::mem::size_of::<FrameUniformBuffer>() as _,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            },
        );

        let view_from_world = config.view_from_world.to_mat4();
        let camera_position = config.view_from_world.inverse().translation();
        let camera_direction = view_from_world.row(2).truncate();
        let aspect_ratio =
            config.resolution_in_pixel[0] as f32 / config.resolution_in_pixel[1] as f32;

        let (projection_from_view, tan_half_fov, pixel_world_size_from_camera_distance) =
            match config.projection_from_view {
                Projection::Perspective {
                    vertical_fov,
                    near_plane_distance,
                } => {
                    // We use infinite reverse-z projection matrix
                    // * great precision both with floating point and integer: https://developer.nvidia.com/content/depth-precision-visualized
                    // * no need to worry about far plane
                    let projection_from_view = glam::Mat4::perspective_infinite_reverse_rh(
                        vertical_fov,
                        aspect_ratio,
                        near_plane_distance,
                    );

                    // Calculate ratio between screen size and screen distance.
                    // Great for getting directions from normalized device coordinates.
                    // (btw. this is the same as [1.0 / projection_from_view[0].x, 1.0 / projection_from_view[1].y])
                    let tan_half_fov = glam::vec2(
                        (vertical_fov * 0.5).tan() * aspect_ratio,
                        (vertical_fov * 0.5).tan(),
                    );

                    // Determine how wide a pixel is in world space at unit distance from the camera.
                    //
                    // derivation:
                    // tan(FOV / 2) = (screen_in_world / 2) / distance
                    // screen_in_world = tan(FOV / 2) * distance * 2
                    //
                    // want: pixels in world per distance, i.e (screen_in_world / resolution / distance)
                    // => (resolution / screen_in_world / distance) = tan(FOV / 2) * distance * 2 / resolution / distance =
                    //                                              = tan(FOV / 2) * 2.0 / resolution
                    let pixel_world_size_from_camera_distance =
                        tan_half_fov.y * 2.0 / config.resolution_in_pixel[1] as f32;

                    (
                        projection_from_view,
                        tan_half_fov,
                        pixel_world_size_from_camera_distance,
                    )
                }
                Projection::Orthographic {
                    vertical_world_size,
                    far_plane_distance,
                } => {
                    let horizontal_world_size = vertical_world_size * aspect_ratio;
                    let projection_from_view = glam::Mat4::orthographic_rh(
                        -0.5 * horizontal_world_size,
                        0.5 * horizontal_world_size,
                        -0.5 * vertical_world_size,
                        0.5 * vertical_world_size,
                        // Consistent perspective projection, we inverse z (here by swapping near and far plane).
                        far_plane_distance,
                        0.0,
                    );

                    let tan_half_fov = glam::vec2(f32::INFINITY, f32::INFINITY);
                    let pixel_world_size_from_camera_distance =
                        vertical_world_size / config.resolution_in_pixel[1] as f32;

                    (
                        projection_from_view,
                        tan_half_fov,
                        pixel_world_size_from_camera_distance,
                    )
                }
            };

        let projection_from_world = projection_from_view * view_from_world;

        ctx.queue.write_buffer(
            ctx.resource_pools
                .buffers
                .get_resource(&frame_uniform_buffer)
                .unwrap(),
            0,
            bytemuck::bytes_of(&FrameUniformBuffer {
                view_from_world: glam::Affine3A::from_mat4(view_from_world).into(),
                projection_from_view: projection_from_view.into(),
                projection_from_world: projection_from_world.into(),
                camera_position: camera_position.into(),
                camera_direction: camera_direction.into(),
                tan_half_fov: tan_half_fov.into(),
                pixel_world_size_from_camera_distance,
                _padding: 0.0,
            }),
        );

        let bind_group_0 = ctx.shared_renderer_data.global_bindings.create_bind_group(
            &mut ctx.resource_pools,
            &ctx.device,
            &frame_uniform_buffer,
        );

        self.setup = Some(ViewTargetSetup {
            name: config.name,
            tonemapping_drawable,
            bind_group_0,
            main_target_msaa: hdr_render_target_msaa,
            main_target_resolved,
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
        crate::profile_function!();

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
    pub fn draw(&mut self, ctx: &RenderContext) -> anyhow::Result<wgpu::CommandBuffer> {
        crate::profile_function!();

        let setup = self
            .setup
            .as_ref()
            .context("ViewBuilder::setup_view wasn't called yet")?;

        let color_msaa = ctx
            .resource_pools
            .textures
            .get_resource(&setup.main_target_msaa)
            .context("hdr render target msaa")?;
        let color_resolved = ctx
            .resource_pools
            .textures
            .get_resource(&setup.main_target_resolved)
            .context("hdr render target resolved")?;
        let depth = ctx
            .resource_pools
            .textures
            .get_resource(&setup.depth_buffer)
            .context("depth buffer")?;

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: setup.name.clone().get(),
            });

        {
            crate::profile_scope!("view builder main target pass");

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: DebugLabel::from(format!("{:?} - main pass", setup.name)).get(),
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
                ctx.resource_pools
                    .bind_groups
                    .get_resource(&setup.bind_group_0)
                    .context("get global bind group")?,
                &[],
            );

            self.queued_draws
                .sort_by(|a, b| a.sorting_index.cmp(&b.sorting_index));
            for queued_draw in &self.queued_draws {
                (queued_draw.draw_func)(ctx, &mut pass).context("drawing a view")?;
            }
        }

        Ok(encoder.finish())
    }

    /// Applies tonemapping and draws the final result of a `ViewBuilder` to a given output `RenderPass`.
    ///
    /// The bound surface(s) on the `RenderPass` are expected to be the same format as specified on `Context` creation.
    pub fn composite<'a>(
        self,
        ctx: &'a RenderContext,
        pass: &mut wgpu::RenderPass<'a>,
    ) -> anyhow::Result<()> {
        crate::profile_function!();

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
            ctx.resource_pools
                .bind_groups
                .get_resource(&setup.bind_group_0)
                .context("get global bind group")?,
            &[],
        );

        let tonemapper = ctx
            .renderers
            .get::<Compositor>()
            .context("get compositor")?;
        tonemapper
            .draw(&ctx.resource_pools, pass, &setup.tonemapping_drawable)
            .context("composite into main view")
    }
}
