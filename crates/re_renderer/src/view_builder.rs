use anyhow::{Context, Ok};
use parking_lot::RwLock;
use std::sync::Arc;

use crate::{
    allocator::{create_and_fill_uniform_buffer, GpuReadbackBuffer, GpuReadbackBufferIdentifier},
    context::RenderContext,
    global_bindings::FrameUniformBuffer,
    renderer::{
        CompositorDrawData, DrawData, DrawPhase, OutlineConfig, OutlineMaskProcessor, Renderer,
    },
    wgpu_resources::{
        texture_row_data_info, GpuBindGroup, GpuTexture, TextureDesc, TextureRowDataInfo,
    },
    DebugLabel, Rgba, Size,
};

type DrawFn = dyn for<'a, 'b> Fn(
        &'b RenderContext,
        DrawPhase,
        &'a mut wgpu::RenderPass<'b>,
        &'b dyn std::any::Any,
    ) -> anyhow::Result<()>
    + Sync
    + Send;

struct QueuedDraw {
    draw_func: Box<DrawFn>,
    draw_data: Box<dyn std::any::Any + std::marker::Send + std::marker::Sync>,
    renderer_name: &'static str,
    participated_phases: &'static [DrawPhase],
}

/// The highest level rendering block in `re_renderer`.
/// Used to build up/collect various resources and then send them off for rendering of  a single view.
#[derive(Default)]
pub struct ViewBuilder {
    /// Result of [`ViewBuilder::setup_view`] - needs to be `Option` sine some of the fields don't have a default.
    setup: Option<ViewTargetSetup>,
    queued_draws: Vec<QueuedDraw>,

    // TODO(andreas): Consider making "render processors" a "thing" by establishing a form of hardcoded/limited-flexibility render-graph
    outline_mask_processor: Option<OutlineMaskProcessor>,

    scheduled_screenshot: Option<GpuReadbackBuffer>,
}

struct ViewTargetSetup {
    name: DebugLabel,

    bind_group_0: GpuBindGroup,
    main_target_msaa: GpuTexture,
    main_target_resolved: GpuTexture,
    depth_buffer: GpuTexture,

    resolution_in_pixel: [u32; 2],
}

/// [`ViewBuilder`] that can be shared between threads.
///
/// Innermost field is an Option, so it can be consumed for `composite`.
pub type SharedViewBuilder = Arc<RwLock<Option<ViewBuilder>>>;

/// Configures the camera placement in the orthographic frustum,
/// as well as the coordinate system convention.
#[derive(Debug, Clone)]
pub enum OrthographicCameraMode {
    /// Puts the view space origin into the middle of the screen.
    ///
    /// Near plane is at z==0, everything with view space z>0 is clipped.
    ///
    /// This is best for regular 3D content.
    ///
    /// Uses `RUB` (X=Right, Y=Up, Z=Back)
    NearPlaneCenter,

    /// Puts the view space origin at the top-left corner of the orthographic frustum and inverts the y axis,
    /// such that the bottom-right corner is at `glam::vec3(vertical_world_size * aspect_ratio, vertical_world_size, 0.0)` in view space.
    ///
    /// Near plane is at z==-far_plane_distance, allowing the same z range both negative and positive.
    ///
    /// This means that for an identity camera, world coordinates map directly to pixel coordinates
    /// (if [`Projection::Orthographic::vertical_world_size`] is set to the y resolution).
    /// Best for pure 2D content.
    ///
    /// Uses `RDF` (X=Right, Y=Down, Z=Forward)
    TopLeftCornerAndExtendZ,
}

/// How we project from 3D to 2D.
#[derive(Debug, Clone)]
pub enum Projection {
    /// Perspective camera looking along the negative z view space axis.
    Perspective {
        /// Viewing angle in view space y direction (which is the vertical screen axis) in radian.
        vertical_fov: f32,

        /// Distance of the near plane.
        near_plane_distance: f32,
    },

    /// Orthographic projection with the camera position at the near plane's center,
    /// looking along the negative z view space axis.
    Orthographic {
        camera_mode: OrthographicCameraMode,

        /// Size of the orthographic camera view space y direction (which is the vertical screen axis).
        vertical_world_size: f32,

        /// Distance of the far plane to the camera.
        far_plane_distance: f32,
    },
}

/// How [`Size::AUTO`] is interpreted.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct AutoSizeConfig {
    /// Determines the point radius when [`crate::Size::AUTO`].
    ///
    /// If this in turn is an auto size, re_renderer will pick an arbitrary but okish ui size.
    pub point_radius: Size,

    /// Determines the line radius for [`crate::Size::AUTO`] for lines.
    ///
    /// If this in turn is an auto size, re_renderer will pick an arbitrary but okish ui size.
    pub line_radius: Size,
}

impl Default for AutoSizeConfig {
    fn default() -> Self {
        Self {
            point_radius: Size::AUTO,
            line_radius: Size::AUTO,
        }
    }
}

/// Basic configuration for a target view.
#[derive(Debug, Clone)]
pub struct TargetConfiguration {
    pub name: DebugLabel,
    pub resolution_in_pixel: [u32; 2],
    pub view_from_world: macaw::IsoTransform,
    pub projection_from_view: Projection,

    /// How many pixels are there per point.
    /// I.e. the ui scaling factor.
    pub pixels_from_point: f32,

    /// How [`Size::AUTO`] is interpreted.
    pub auto_size_config: AutoSizeConfig,

    pub outline_config: Option<OutlineConfig>,
}

impl Default for TargetConfiguration {
    fn default() -> Self {
        Self {
            name: "default view".into(),
            resolution_in_pixel: [100, 100],
            view_from_world: Default::default(),
            projection_from_view: Projection::Perspective {
                vertical_fov: 70.0 * std::f32::consts::TAU / 360.0,
                near_plane_distance: 0.01,
            },
            pixels_from_point: 1.0,
            auto_size_config: Default::default(),
            outline_config: None,
        }
    }
}

pub struct ScheduledScreenshot {
    pub identifier: GpuReadbackBufferIdentifier,
    pub width: u32,
    pub height: u32,
    pub row_info: TextureRowDataInfo,
}

impl ViewBuilder {
    /// Color format used for the main target of the view builder.
    ///
    /// Eventually we'll want to make this an HDR format and apply tonemapping during composite.
    /// However, note that it is easy to run into subtle MSAA quality issues then:
    /// Applying MSAA resolve before tonemapping is problematic as it means we're doing msaa in linear.
    /// This is especially problematic at bright/dark edges where we may loose "smoothness"!
    /// For a nice illustration see [this blog post by MRP](https://therealmjp.github.io/posts/msaa-overview/)
    /// We either would need to keep the MSAA target and tonemap it, or
    /// apply a manual resolve where we inverse-tonemap non-fully-covered pixel before averaging.
    /// (an optimized variant of this is described [by AMD here](https://gpuopen.com/learn/optimized-reversible-tonemapper-for-resolve/))
    /// In any case, this gets us onto a potentially much costlier rendering path, especially for tiling GPUs.
    pub const MAIN_TARGET_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

    /// The texture format used for screenshots.
    pub const SCREENSHOT_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    /// Depth format used for the main target of the view builder.
    ///
    /// [`wgpu::TextureFormat::Depth24Plus`] would be preferable for performance, see [Nvidia's Vulkan dos and dont's](https://developer.nvidia.com/blog/vulkan-dos-donts/).
    /// However, the problem with being "24Plus" is that we no longer know what format we'll actually get, which is a problem e.g. for vertex shader determined depth offsets.
    /// (This is a real concern - for example on Metal we always get a floating point target with this!)
    /// [`wgpu::TextureFormat::Depth32Float`] on the other hand is widely supported and has the best possible precision (with reverse infinite z projection which we're already using).
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

    /// Default value for clearing depth buffer to infinity.
    ///
    /// 0.0 == far since we're using reverse-z.
    pub const DEFAULT_DEPTH_CLEAR: wgpu::LoadOp<f32> = wgpu::LoadOp::Clear(0.0);

    /// Default depth state for enabled depth write & read.
    pub const MAIN_TARGET_DEFAULT_DEPTH_STATE: Option<wgpu::DepthStencilState> =
        Some(wgpu::DepthStencilState {
            format: Self::MAIN_TARGET_DEPTH_FORMAT,
            depth_compare: wgpu::CompareFunction::Greater,
            depth_write_enabled: true,
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState::IGNORE,
                back: wgpu::StencilFaceState::IGNORE,
                read_mask: 0,
                write_mask: 0,
            },
            bias: wgpu::DepthBiasState {
                constant: 0,
                slope_scale: 0.0,
                clamp: 0.0,
            },
        });

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
            .gpu_resources
            .textures
            .alloc(&ctx.device, &main_target_desc);
        // Like hdr_render_target, but with MSAA resolved.
        let main_target_resolved = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: format!("{:?} - main target resolved", config.name).into(),
                sample_count: 1,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                ..main_target_desc
            },
        );
        let depth_buffer = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: format!("{:?} - depth buffer", config.name).into(),
                format: Self::MAIN_TARGET_DEPTH_FORMAT,
                ..main_target_desc
            },
        );

        self.outline_mask_processor = config.outline_config.as_ref().map(|outline_config| {
            OutlineMaskProcessor::new(
                ctx,
                outline_config,
                &config.name,
                config.resolution_in_pixel,
            )
        });

        self.queue_draw(&CompositorDrawData::new(
            ctx,
            &main_target_resolved,
            self.outline_mask_processor
                .as_ref()
                .map(|p| p.final_voronoi_texture()),
            &config.outline_config,
        ));

        let aspect_ratio =
            config.resolution_in_pixel[0] as f32 / config.resolution_in_pixel[1] as f32;

        let (projection_from_view, tan_half_fov, pixel_world_size_from_camera_distance) =
            match config.projection_from_view.clone() {
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
                    camera_mode,
                    vertical_world_size,
                    far_plane_distance,
                } => {
                    let horizontal_world_size = vertical_world_size * aspect_ratio;
                    // Note that we inverse z (by swapping near and far plane) to be consistent with our perspective projection.
                    let projection_from_view = match camera_mode {
                        OrthographicCameraMode::NearPlaneCenter => glam::Mat4::orthographic_rh(
                            -0.5 * horizontal_world_size,
                            0.5 * horizontal_world_size,
                            -0.5 * vertical_world_size,
                            0.5 * vertical_world_size,
                            far_plane_distance,
                            0.0,
                        ),
                        OrthographicCameraMode::TopLeftCornerAndExtendZ => {
                            glam::Mat4::orthographic_rh(
                                0.0,
                                horizontal_world_size,
                                vertical_world_size,
                                0.0,
                                far_plane_distance,
                                -far_plane_distance,
                            )
                        }
                    };

                    let tan_half_fov = glam::vec2(f32::MAX, f32::MAX);
                    let pixel_world_size_from_camera_distance =
                        vertical_world_size / config.resolution_in_pixel[1] as f32;

                    (
                        projection_from_view,
                        tan_half_fov,
                        pixel_world_size_from_camera_distance,
                    )
                }
            };

        let mut view_from_world = config.view_from_world.to_mat4();
        // For OrthographicCameraMode::TopLeftCorner, we want Z facing forward.
        match config.projection_from_view {
            Projection::Orthographic { camera_mode, .. } => match camera_mode {
                OrthographicCameraMode::TopLeftCornerAndExtendZ => {
                    *view_from_world.col_mut(2) = -view_from_world.col(2);
                }
                OrthographicCameraMode::NearPlaneCenter => {}
            },
            Projection::Perspective { .. } => {}
        };
        let camera_position = config.view_from_world.inverse().translation();
        let camera_forward = -view_from_world.row(2).truncate();
        let projection_from_world = projection_from_view * view_from_world;

        let auto_size_points = if config.auto_size_config.point_radius.is_auto() {
            Size::new_points(2.5)
        } else {
            config.auto_size_config.point_radius
        };
        let auto_size_lines = if config.auto_size_config.line_radius.is_auto() {
            Size::new_points(1.5)
        } else {
            config.auto_size_config.line_radius
        };

        // Setup frame uniform buffer
        let frame_uniform_buffer = create_and_fill_uniform_buffer(
            ctx,
            format!("{:?} - frame uniform buffer", config.name).into(),
            FrameUniformBuffer {
                view_from_world: glam::Affine3A::from_mat4(view_from_world).into(),
                projection_from_view: projection_from_view.into(),
                projection_from_world: projection_from_world.into(),
                camera_position,
                camera_forward,
                tan_half_fov: tan_half_fov.into(),
                pixel_world_size_from_camera_distance,
                pixels_from_point: config.pixels_from_point,

                auto_size_points: auto_size_points.0,
                auto_size_lines: auto_size_lines.0,

                end_padding: Default::default(),
            },
        );

        let bind_group_0 = ctx.shared_renderer_data.global_bindings.create_bind_group(
            &mut ctx.gpu_resources,
            &ctx.device,
            frame_uniform_buffer,
        );

        self.setup = Some(ViewTargetSetup {
            name: config.name,
            bind_group_0,
            main_target_msaa: hdr_render_target_msaa,
            main_target_resolved,
            depth_buffer,
            resolution_in_pixel: config.resolution_in_pixel,
        });

        Ok(self)
    }

    fn draw_phase<'a>(
        &'a self,
        ctx: &'a RenderContext,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'a>,
    ) {
        crate::profile_function!();

        for queued_draw in &self.queued_draws {
            if queued_draw.participated_phases.contains(&phase) {
                let res = (queued_draw.draw_func)(ctx, phase, pass, queued_draw.draw_data.as_ref())
                    .with_context(|| format!("draw call during phase {phase:?}"));
                if let Err(err) = res {
                    re_log::error!(renderer=%queued_draw.renderer_name, %err,
                        "renderer failed to draw");
                }
            }
        }
    }

    pub fn queue_draw<D: DrawData + Sync + Send + Clone + 'static>(
        &mut self,
        draw_data: &D,
    ) -> &mut Self {
        crate::profile_function!();
        self.queued_draws.push(QueuedDraw {
            draw_func: Box::new(move |ctx, phase, pass, draw_data| {
                let renderers = ctx.renderers.read();
                let renderer = renderers
                    .get::<D::Renderer>()
                    .context("failed to retrieve renderer")?;
                let draw_data = draw_data
                    .downcast_ref::<D>()
                    .expect("passed wrong type of draw data");
                renderer.draw(&ctx.gpu_resources, phase, pass, draw_data)
            }),
            draw_data: Box::new(draw_data.clone()),
            renderer_name: std::any::type_name::<D::Renderer>(),
            participated_phases: D::Renderer::participated_phases(),
        });

        self
    }

    /// Draws the frame as instructed to a temporary HDR target.
    pub fn draw(
        &mut self,
        ctx: &RenderContext,
        clear_color: Rgba,
    ) -> anyhow::Result<wgpu::CommandBuffer> {
        crate::profile_function!();

        let setup = self
            .setup
            .as_ref()
            .context("ViewBuilder::setup_view wasn't called yet")?;

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: setup.name.clone().get(),
            });

        let clear_color = wgpu::Color {
            r: clear_color.r() as f64,
            g: clear_color.g() as f64,
            b: clear_color.b() as f64,
            a: clear_color.a() as f64,
        };

        {
            crate::profile_scope!("main target pass");

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: setup.name.clone().push_str(" - main pass").get(),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &setup.main_target_msaa.default_view,
                    resolve_target: Some(&setup.main_target_resolved.default_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        // Don't care about the result, it's going to be resolved to the resolve target.
                        // This can have be much better perf, especially on tiler gpus.
                        store: false,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &setup.depth_buffer.default_view,
                    depth_ops: Some(wgpu::Operations {
                        load: Self::DEFAULT_DEPTH_CLEAR,
                        store: false,
                    }),
                    stencil_ops: None,
                }),
            });

            pass.set_bind_group(0, &setup.bind_group_0, &[]);

            for phase in [DrawPhase::Opaque, DrawPhase::Background] {
                self.draw_phase(ctx, phase, &mut pass);
            }
        }

        if let Some(outline_mask_processor) = self.outline_mask_processor.take() {
            crate::profile_scope!("outlines");
            {
                crate::profile_scope!("outline mask pass");
                let mut pass = outline_mask_processor.start_mask_render_pass(&mut encoder);
                pass.set_bind_group(0, &setup.bind_group_0, &[]);
                self.draw_phase(ctx, DrawPhase::OutlineMask, &mut pass);
            }
            outline_mask_processor.compute_outlines(&ctx.gpu_resources, &mut encoder)?;
        }

        // Execute compositing into a special target if we want to take a screenshot.
        // This is necessary because `composite` is expected to write directly into the final output
        // from which we can't read back and of which we don't control the format.
        //
        // This comes with the perk that we can do extra things here if we want!
        //
        // TODO(andreas): Like more antialiasing!
        // We could render the same image with subpixel moved camera in order to get super-sampling without hitting texture size limitations.
        // Or alternatively try to render the images in several tiles 🤔. In any case this would greatly improve quality!
        if let Some(screenshot_target_buffer) = self.scheduled_screenshot.take() {
            let screenshot_texture = ctx.gpu_resources.textures.alloc(
                &ctx.device,
                &TextureDesc {
                    label: setup.name.clone().push_str(" - screenshot target"),
                    size: wgpu::Extent3d {
                        width: setup.resolution_in_pixel[0],
                        height: setup.resolution_in_pixel[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: Self::SCREENSHOT_COLOR_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                },
            );

            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: setup.name.clone().push_str(" - screenshot").get(),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &screenshot_texture.default_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(clear_color),
                            store: true,
                        },
                    })],
                    depth_stencil_attachment: None,
                });

                pass.set_bind_group(0, &setup.bind_group_0, &[]);
                self.draw_phase(ctx, DrawPhase::CompositingScreenshot, &mut pass);
            }

            let bytes_per_row = texture_row_data_info(
                screenshot_texture.texture.format(),
                screenshot_texture.texture.width(),
            )
            .bytes_per_row_padded;
            screenshot_target_buffer.read_texture(
                &mut encoder,
                wgpu::ImageCopyTexture {
                    texture: &screenshot_texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                std::num::NonZeroU32::new(bytes_per_row),
                None,
                screenshot_texture.texture.size(),
            );
        }

        Ok(encoder.finish())
    }

    /// Schedules the taking of a screenshot.
    ///
    /// Needs to be called after setup.
    /// Returns screenshot data properties for convenience.
    ///
    /// TODO: Document how to get the data
    pub fn schedule_screenshot(
        &mut self,
        ctx: &RenderContext,
    ) -> anyhow::Result<ScheduledScreenshot> {
        if self.scheduled_screenshot.is_some() {
            anyhow::bail!("A screenshot is already scheduled");
        };

        let setup = self
            .setup
            .as_ref()
            .context("ViewBuilder::setup_view wasn't called yet")?;

        let row_info =
            texture_row_data_info(Self::SCREENSHOT_COLOR_FORMAT, setup.resolution_in_pixel[0]);
        let buffer_size = row_info.bytes_per_row_padded * setup.resolution_in_pixel[1];
        let screenshot_buffer = ctx.gpu_readback_belt.lock().allocate(
            &ctx.device,
            &ctx.gpu_resources.buffers,
            buffer_size as u64,
        );

        let identifier = screenshot_buffer.identifier;
        self.scheduled_screenshot = Some(screenshot_buffer);

        Ok(ScheduledScreenshot {
            row_info,
            identifier,
            width: setup.resolution_in_pixel[0],
            height: setup.resolution_in_pixel[1],
        })
    }

    /// Composites the final result of a `ViewBuilder` to a given output `RenderPass`.
    ///
    /// The bound surface(s) on the `RenderPass` are expected to be the same format as specified on `Context` creation.
    /// `screen_position` specifies where on the output pass the view is placed.
    pub fn composite<'a>(
        &'a self,
        ctx: &'a RenderContext,
        pass: &mut wgpu::RenderPass<'a>,
        screen_position: glam::Vec2,
    ) -> anyhow::Result<()> {
        crate::profile_function!();

        let setup = self
            .setup
            .as_ref()
            .context("ViewBuilder::setup_view wasn't called yet")?;

        pass.set_viewport(
            screen_position.x,
            screen_position.y,
            setup.resolution_in_pixel[0] as f32,
            setup.resolution_in_pixel[1] as f32,
            0.0,
            1.0,
        );

        pass.set_bind_group(0, &setup.bind_group_0, &[]);
        self.draw_phase(ctx, DrawPhase::Compositing, pass);

        Ok(())
    }
}
