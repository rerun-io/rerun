use parking_lot::RwLock;
use std::sync::Arc;

use crate::{
    allocator::{create_and_fill_uniform_buffer, GpuReadbackIdentifier},
    context::{RenderContext, Renderers},
    draw_phases::{
        DrawPhase, OutlineConfig, OutlineMaskProcessor, PickingLayerError, PickingLayerProcessor,
        ScreenshotProcessor,
    },
    global_bindings::FrameUniformBuffer,
    queueable_draw_data::QueueableDrawData,
    renderer::{CompositorDrawData, DebugOverlayDrawData},
    transform::RectTransform,
    wgpu_resources::{
        GpuBindGroup, GpuRenderPipelinePoolAccessor, GpuTexture, PoolError, TextureDesc,
    },
    DebugLabel, RectInt, Rgba,
};

#[derive(thiserror::Error, Debug)]
pub enum ViewBuilderError {
    #[error("Screenshot was already scheduled.")]
    ScreenshotAlreadyScheduled,

    #[error("Picking rectangle readback was already scheduled.")]
    PickingRectAlreadyScheduled,

    #[error(transparent)]
    InvalidDebugOverlay(#[from] crate::renderer::DebugOverlayError),
}

/// The highest level rendering block in `re_renderer`.
/// Used to build up/collect various resources and then send them off for rendering of a single view.
pub struct ViewBuilder {
    setup: ViewTargetSetup,
    queued_draws: Vec<QueueableDrawData>,

    // TODO(andreas): Consider making "render processors" a "thing" by establishing a form of hardcoded/limited-flexibility render-graph
    outline_mask_processor: Option<OutlineMaskProcessor>,
    screenshot_processor: Option<ScreenshotProcessor>,
    picking_processor: Option<PickingLayerProcessor>,
}

struct ViewTargetSetup {
    name: DebugLabel,

    bind_group_0: GpuBindGroup,
    main_target_msaa: GpuTexture,
    main_target_resolved: GpuTexture,
    depth_buffer: GpuTexture,

    frame_uniform_buffer_content: FrameUniformBuffer,

    resolution_in_pixel: [u32; 2],
}

/// [`ViewBuilder`] that can be shared between threads.
///
/// Innermost field is an Option, so it can be consumed for `composite`.
pub type SharedViewBuilder = Arc<RwLock<Option<ViewBuilder>>>;

/// Configures the camera placement in the orthographic frustum,
/// as well as the coordinate system convention.
#[derive(Debug, Clone, Copy)]
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
#[derive(Debug, Clone, Copy)]
pub enum Projection {
    /// Perspective camera looking along the negative z view space axis.
    Perspective {
        /// Viewing angle in view space y direction (which is the vertical screen axis) in radian.
        vertical_fov: f32,

        /// Distance of the near plane.
        near_plane_distance: f32,

        /// Aspect ratio of the perspective transformation.
        ///
        /// This is typically just resolution.y / resolution.x.
        /// Setting this to anything else is mostly useful when panning & zooming within a fixed transformation.
        aspect_ratio: f32,
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

impl Projection {
    fn projection_from_view(self, resolution_in_pixel: [u32; 2]) -> glam::Mat4 {
        match self {
            Self::Perspective {
                vertical_fov,
                near_plane_distance,
                aspect_ratio,
            } => {
                // We use infinite reverse-z projection matrix
                // * great precision both with floating point and integer: https://developer.nvidia.com/content/depth-precision-visualized
                // * no need to worry about far plane
                glam::Mat4::perspective_infinite_reverse_rh(
                    vertical_fov,
                    aspect_ratio,
                    near_plane_distance,
                )
            }
            Self::Orthographic {
                camera_mode,
                vertical_world_size,
                far_plane_distance,
            } => {
                let aspect_ratio = resolution_in_pixel[0] as f32 / resolution_in_pixel[1] as f32;
                let horizontal_world_size = vertical_world_size * aspect_ratio;

                // Note that we inverse z (by swapping near and far plane) to be consistent with our perspective projection.
                match camera_mode {
                    OrthographicCameraMode::NearPlaneCenter => glam::Mat4::orthographic_rh(
                        -0.5 * horizontal_world_size,
                        0.5 * horizontal_world_size,
                        -0.5 * vertical_world_size,
                        0.5 * vertical_world_size,
                        far_plane_distance,
                        0.0,
                    ),
                    OrthographicCameraMode::TopLeftCornerAndExtendZ => glam::Mat4::orthographic_rh(
                        0.0,
                        horizontal_world_size,
                        vertical_world_size,
                        0.0,
                        far_plane_distance,
                        -far_plane_distance,
                    ),
                }
            }
        }
    }

    fn tan_half_fov(&self) -> glam::Vec2 {
        match self {
            Self::Perspective {
                vertical_fov,
                aspect_ratio,
                ..
            } => {
                // Calculate ratio between screen size and screen distance.
                // Great for getting directions from normalized device coordinates.
                // (btw. this is the same as [1.0 / projection_from_view[0].x, 1.0 / projection_from_view[1].y])
                glam::vec2(
                    (vertical_fov * 0.5).tan() * aspect_ratio,
                    (vertical_fov * 0.5).tan(),
                )
            }
            Self::Orthographic { .. } => glam::vec2(f32::MAX, f32::MAX), // Can't use infinity in shaders
        }
    }
}

/// Basic configuration for a target view.
#[derive(Debug, Clone)]
pub struct TargetConfiguration {
    pub name: DebugLabel,

    /// The viewport resolution in physical pixels.
    pub resolution_in_pixel: [u32; 2],
    pub view_from_world: re_math::IsoTransform,
    pub projection_from_view: Projection,

    /// Defines a viewport transformation from the projected space to the final image space.
    ///
    /// This can be used to implement pan & zoom independent of the camera projection.
    /// Meaning that this transform allows you to zoom in on a portion of a perspectively projected
    /// scene.
    ///
    /// Note only the relation of the rectangles in `RectTransform` is important.
    /// Scaling or moving both rectangles by the same amount does not change the result.
    ///
    /// Internally, this is used to transform the normalized device coordinates to the given portion.
    /// This transform is applied to the projection matrix.
    pub viewport_transformation: RectTransform,

    /// How many pixels are there per point.
    ///
    /// I.e. the UI zoom factor.
    /// Note that this does not affect any of the camera & projection properties and is only used
    /// whenever point sizes were explicitly specified.
    pub pixels_per_point: f32,

    pub outline_config: Option<OutlineConfig>,

    /// If true, the `composite` step will blend the image with the background.
    ///
    /// Otherwise, this step will overwrite whatever was there before, drawing the view builder's result
    /// as an opaque rectangle.
    pub blend_with_background: bool,
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
                aspect_ratio: 1.0,
            },
            viewport_transformation: RectTransform::IDENTITY,
            pixels_per_point: 1.0,
            outline_config: None,
            blend_with_background: false,
        }
    }
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

    /// Use this color state when targeting the main target with alpha-to-coverage.
    ///
    /// If blending with the background is enabled, we need alpha to indicate how much we overwrite the background.
    /// (i.e. when we do blending of the screen target with whatever was there during [`Self::composite`].)
    /// However, when using alpha-to-coverage, we need alpha to _also_ indicate the coverage of the pixel from
    /// which the samples are derived. What we'd like to happen is:
    /// * use alpha to indicate coverage == number of samples written to
    /// * write alpha==1.0 for each active sample despite what we set earlier
    ///
    /// This way, we'd get the correct alpha and end up with pre-multipltiplied color values during MSAA resolve,
    /// just like with opaque geometry!
    /// OpenGL exposes this as `GL_SAMPLE_ALPHA_TO_ONE`, Vulkan as `alphaToOne`. Unfortunately though, WebGPU does not support this!
    /// Instead, what happens is that alpha has a double meaning: Coverage _and_ alpha of all written samples.
    /// This means that anti-aliased edges (== alpha < 1.0) will _always_ creates "holes" into the target texture
    /// even if there was already an opaque object prior.
    /// To work around this, we accumulate alpha values with an additive blending operation, so that previous opaque
    /// objects won't be overwritten with alpha < 1.0. (this is obviously wrong for a variety of reasons, but it looks good enough)
    /// Another problem with this is that during MSAA resolve we now average those too low alpha values.
    /// This makes us end up with a premultiplied alpha value that looks like it has additive blending applied since
    /// the resulting alpha value is not what was used to determine the color!
    /// -> See workaround in `composite.wgsl`
    ///
    /// Ultimately, we have the following options to fix this properly sorted from most desirable to least:
    /// * don't use alpha-to-coverage, use instead `SampleMask`
    ///     * this is not supported on WebGL which either needs a special path, or more likely, has to just disable anti-aliasing in these cases
    ///     * as long as we use 4x MSAA, we have a pretty good idea where the samples are (see `jumpflooding_init_msaa.wgsl`),
    ///       so we can actually use this to **improve** the quality of the anti-aliasing a lot by turning on/off the samples that are actually covered.
    /// * figure out a way to never needing to blend with the background in [`Self::composite`].
    /// * figure out how to use `GL_SAMPLE_ALPHA_TO_ONE` after all. This involves bringing this up with the WebGPU spec team and won't work on WebGL.
    pub const MAIN_TARGET_ALPHA_TO_COVERAGE_COLOR_STATE: wgpu::ColorTargetState =
        wgpu::ColorTargetState {
            format: Self::MAIN_TARGET_COLOR_FORMAT,
            blend: Some(wgpu::BlendState {
                color: wgpu::BlendComponent::REPLACE,
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            write_mask: wgpu::ColorWrites::ALL,
        };

    /// The texture format used for screenshots.
    pub const SCREENSHOT_COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    /// Depth format used for the main target of the view builder.
    ///
    /// [`wgpu::TextureFormat::Depth24Plus`] would be preferable for performance, see [Nvidia's Vulkan dos and don'ts](https://developer.nvidia.com/blog/vulkan-dos-donts/).
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
        count: Self::MAIN_TARGET_SAMPLE_COUNT,
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
            // It's important to set the depth test to GreaterEqual, not to Greater.
            // This way, we ensure that objects that are drawn later with the exact same depth value, can overwrite earlier ones!
            depth_compare: wgpu::CompareFunction::GreaterEqual,
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

    pub fn new(ctx: &RenderContext, config: TargetConfiguration) -> Self {
        re_tracing::profile_function!();

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

        let projection_from_view = config
            .projection_from_view
            .projection_from_view(config.resolution_in_pixel);

        let tan_half_fov = config.projection_from_view.tan_half_fov();

        let resolution = glam::Vec2::new(
            config.resolution_in_pixel[0] as f32,
            config.resolution_in_pixel[1] as f32,
        );
        let pixel_world_size_from_camera_distance = match config.projection_from_view {
            Projection::Perspective { .. } => {
                // Determine how wide a pixel is in world space at unit distance from the camera.
                //
                // derivation:
                // tan(FOV / 2) = (screen_in_world / 2) / distance
                // screen_in_world = tan(FOV / 2) * distance * 2
                //
                // want: pixels in world per distance, i.e (screen_in_world / resolution / distance)
                // => (resolution / screen_in_world / distance) = tan(FOV / 2) * distance * 2 / resolution / distance =
                //                                              = tan(FOV / 2) * 2.0 / resolution
                tan_half_fov * 2.0 / resolution
            }
            Projection::Orthographic {
                vertical_world_size,
                ..
            } => {
                glam::vec2(
                    vertical_world_size * resolution.x / resolution.y,
                    vertical_world_size,
                ) / resolution
            }
        };

        // Finally, apply a viewport transformation to the projection.
        let ndc_scale_and_translation = config
            .viewport_transformation
            .to_ndc_scale_and_translation();
        let projection_from_view = ndc_scale_and_translation * projection_from_view;

        // Need to take into account that a smaller or bigger portion of the world scale is visible now.
        let pixel_world_size_from_camera_distance =
            pixel_world_size_from_camera_distance * config.viewport_transformation.scale();

        // Unless the transformation intentionally stretches the image,
        // our world size -> pixel size conversation factor should be roughly the same in both directions.
        //
        // As of writing, the shaders dealing with pixel size estimation, can't deal with non-uniform
        // scaling in the viewport transformation.
        let pixel_world_size_from_camera_distance = pixel_world_size_from_camera_distance.x;

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

        // Setup frame uniform buffer
        let frame_uniform_buffer_content = FrameUniformBuffer {
            view_from_world: glam::Affine3A::from_mat4(view_from_world).into(),
            projection_from_view: projection_from_view.into(),
            projection_from_world: projection_from_world.into(),
            camera_position,
            camera_forward,
            tan_half_fov: tan_half_fov.into(),
            pixel_world_size_from_camera_distance,
            pixels_per_point: config.pixels_per_point,

            device_tier: (ctx.device_caps().tier as u32).into(),
        };
        let frame_uniform_buffer = create_and_fill_uniform_buffer(
            ctx,
            format!("{:?} - frame uniform buffer", config.name).into(),
            frame_uniform_buffer_content,
        );

        let bind_group_0 = ctx.global_bindings.create_bind_group(
            &ctx.gpu_resources,
            &ctx.device,
            frame_uniform_buffer,
        );

        let outline_mask_processor = config.outline_config.as_ref().map(|outline_config| {
            OutlineMaskProcessor::new(
                ctx,
                outline_config,
                &config.name,
                config.resolution_in_pixel,
            )
        });

        let composition_draw = CompositorDrawData::new(
            ctx,
            &main_target_resolved,
            outline_mask_processor
                .as_ref()
                .map(|p| p.final_voronoi_texture()),
            &config.outline_config,
            config.blend_with_background,
        );

        let setup = ViewTargetSetup {
            name: config.name,
            bind_group_0,
            main_target_msaa: hdr_render_target_msaa,
            main_target_resolved,
            depth_buffer,
            resolution_in_pixel: config.resolution_in_pixel,
            frame_uniform_buffer_content,
        };

        Self {
            setup,
            queued_draws: vec![composition_draw.into()],
            outline_mask_processor,
            screenshot_processor: Default::default(),
            picking_processor: Default::default(),
        }
    }

    /// Resolution in pixels as configured on view builder creation.
    pub fn resolution_in_pixel(&self) -> [u32; 2] {
        self.setup.resolution_in_pixel
    }

    fn draw_phase(
        &self,
        renderers: &Renderers,
        render_pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        phase: DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
    ) {
        re_tracing::profile_function!();

        for queued_draw in &self.queued_draws {
            if queued_draw.participated_phases.contains(&phase) {
                let res = (queued_draw.draw_func)(
                    renderers,
                    render_pipelines,
                    phase,
                    pass,
                    queued_draw.draw_data.as_ref(),
                );
                if let Err(err) = res {
                    re_log::error!(renderer=%queued_draw.renderer_name, %err,
                        "renderer failed to draw");
                }
            }
        }
    }

    pub fn queue_draw(&mut self, draw_data: impl Into<QueueableDrawData>) -> &mut Self {
        self.queued_draws.push(draw_data.into());
        self
    }

    /// Draws the frame as instructed to a temporary HDR target.
    pub fn draw(
        &self,
        ctx: &RenderContext,
        clear_color: Rgba,
    ) -> Result<wgpu::CommandBuffer, PoolError> {
        re_tracing::profile_function!();

        // Renderers and render pipelines are locked for the entirety of this method:
        // This means it's *not* possible to add renderers or pipelines while drawing is in progress!
        //
        // This is primarily due to the lifetime association render passes have all passed in resources:
        // For dynamic resources like bind groups/textures/buffers we use handles that *store* an arc
        // to the wgpu resources to solve this ownership problem.
        // But for render pipelines, which we want to be able the resource under a handle via reload,
        // so we always have to do some kind of lookup prior to or during rendering.
        // Therefore, we just lock the pool for the entirety of the draw which ensures
        // that the lock outlives the pass.
        //
        // Renderers can't be added anyways at this point (RendererData add their Renderer on creation),
        // so no point in taking the lock repeatedly.
        //
        // TODO(gfx-rs/wgpu#1453): Note that this is a limitation that will be lifted in future versions of wgpu.
        // However, having our locking concentrated for the duration of a view draw
        // is also beneficial since it enforces the model of prepare->draw which avoids a lot of repeated
        // locking and unlocking.
        let renderers = ctx.read_lock_renderers();
        let pipelines = ctx.gpu_resources.render_pipelines.resources();

        let setup = &self.setup;

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: setup.name.clone().get(),
            });

        {
            re_tracing::profile_scope!("main target pass");

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: DebugLabel::from(format!("{} - main pass", setup.name)).get(),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &setup.main_target_msaa.default_view,
                    resolve_target: Some(&setup.main_target_resolved.default_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear_color.r() as f64,
                            g: clear_color.g() as f64,
                            b: clear_color.b() as f64,
                            a: clear_color.a() as f64,
                        }),
                        // Don't care about the result, it's going to be resolved to the resolve target.
                        // This can have be much better perf, especially on tiler gpus.
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &setup.depth_buffer.default_view,
                    depth_ops: Some(wgpu::Operations {
                        load: Self::DEFAULT_DEPTH_CLEAR,
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            pass.set_bind_group(0, &setup.bind_group_0, &[]);

            for phase in [
                DrawPhase::Opaque,
                DrawPhase::Background,
                DrawPhase::Transparent,
            ] {
                self.draw_phase(&renderers, &pipelines, phase, &mut pass);
            }
        }

        if let Some(picking_processor) = &self.picking_processor {
            {
                let mut pass = picking_processor.begin_render_pass(&setup.name, &mut encoder);
                // PickingProcessor has as custom frame uniform buffer.
                //
                // TODO(andreas): Formalize this somehow.
                // Maybe just every processor should have its own and gets abstract information from the view builder to set it up?
                // … or we change this whole thing again so slice things differently:
                // 0: Truly view Global: Samplers, time, point conversions, etc.
                // 1: Phase global (camera & projection goes here)
                // 2: Specific renderer
                // 3: Draw call in renderer.
                //
                //pass.set_bind_group(0, &setup.bind_group_0, &[]);
                self.draw_phase(&renderers, &pipelines, DrawPhase::PickingLayer, &mut pass);
            }
            match picking_processor.end_render_pass(&mut encoder, &pipelines) {
                Err(PickingLayerError::ResourcePoolError(err)) => {
                    return Err(err);
                }
                Err(PickingLayerError::ReadbackError(err)) => {
                    re_log::warn_once!("Failed to schedule picking data readback: {err}");
                }
                Ok(()) => {}
            }
        }

        if let Some(outline_mask_processor) = &self.outline_mask_processor {
            re_tracing::profile_scope!("outlines");
            {
                re_tracing::profile_scope!("outline mask pass");
                let mut pass = outline_mask_processor.start_mask_render_pass(&mut encoder);
                pass.set_bind_group(0, &setup.bind_group_0, &[]);
                self.draw_phase(&renderers, &pipelines, DrawPhase::OutlineMask, &mut pass);
            }
            outline_mask_processor.compute_outlines(&pipelines, &mut encoder)?;
        }

        if let Some(screenshot_processor) = &self.screenshot_processor {
            {
                let mut pass = screenshot_processor.begin_render_pass(&setup.name, &mut encoder);
                pass.set_bind_group(0, &setup.bind_group_0, &[]);
                self.draw_phase(
                    &renderers,
                    &pipelines,
                    DrawPhase::CompositingScreenshot,
                    &mut pass,
                );
            }
            match screenshot_processor.end_render_pass(&mut encoder) {
                Ok(()) => {}
                Err(err) => {
                    re_log::warn_once!("Failed to schedule screenshot data readback: {err}");
                }
            }
        }

        Ok(encoder.finish())
    }

    /// Schedules the taking of a screenshot.
    ///
    /// Needs to be called before [`ViewBuilder::draw`].
    /// Can only be called once per frame per [`ViewBuilder`].
    ///
    /// Data from the screenshot needs to be retrieved via [`crate::ScreenshotProcessor::next_readback_result`].
    /// To do so, you need to pass the exact same `identifier` and type of user data as you've done here:
    /// ```no_run
    /// use re_renderer::{view_builder::ViewBuilder, RenderContext, ScreenshotProcessor};
    /// fn take_screenshot(ctx: &RenderContext, view_builder: &mut ViewBuilder) {
    ///     view_builder.schedule_screenshot(&ctx, 42, "My screenshot".to_owned());
    /// }
    /// fn receive_screenshots(ctx: &RenderContext) {
    ///     while ScreenshotProcessor::next_readback_result::<String>(ctx, 42, |data, extent, user_data| {
    ///             re_log::info!("Received screenshot {}", user_data);
    ///         },
    ///     ).is_some()
    ///     {}
    /// }
    /// ```
    ///
    /// Received data that isn't retrieved for more than a frame will be automatically discarded.
    pub fn schedule_screenshot<T: 'static + Send + Sync>(
        &mut self,
        ctx: &RenderContext,
        identifier: GpuReadbackIdentifier,
        user_data: T,
    ) -> Result<(), ViewBuilderError> {
        if self.screenshot_processor.is_some() {
            return Err(ViewBuilderError::ScreenshotAlreadyScheduled);
        };

        self.screenshot_processor = Some(ScreenshotProcessor::new(
            ctx,
            &self.setup.name,
            self.setup.resolution_in_pixel.into(),
            identifier,
            user_data,
        ));

        Ok(())
    }

    /// Schedules the readback of a rectangle from the picking layer.
    ///
    /// Needs to be called before [`ViewBuilder::draw`].
    /// Can only be called once per frame per [`ViewBuilder`].
    ///
    /// The result will still be valid if the rectangle is partially or fully outside of bounds.
    /// Areas that are not overlapping with the primary target will be filled as-if the view's target was bigger,
    /// i.e. all values are valid picking IDs, it is up to the user to discard anything that is out of bounds.
    ///
    /// Note that the picking layer will not be created in the first place if this isn't called.
    ///
    /// Data from the picking rect needs to be retrieved via [`crate::PickingLayerProcessor::next_readback_result`].
    /// To do so, you need to pass the exact same `identifier` and type of user data as you've done here:
    /// ```no_run
    /// use re_renderer::{view_builder::ViewBuilder, RectInt, PickingLayerProcessor, RenderContext};
    /// fn schedule_picking_readback(
    ///     ctx: &RenderContext,
    ///     view_builder: &mut ViewBuilder,
    ///     picking_rect: RectInt,
    /// ) {
    ///     view_builder.schedule_picking_rect(
    ///         ctx, picking_rect, 42, "My screenshot".to_owned(), false,
    ///     );
    /// }
    /// fn receive_screenshots(ctx: &RenderContext) {
    ///     while let Some(result) = PickingLayerProcessor::next_readback_result::<String>(ctx, 42) {
    ///         re_log::info!("Received picking_data {}", result.user_data);
    ///     }
    /// }
    /// ```
    ///
    /// Received data that isn't retrieved for more than a frame will be automatically discarded.
    pub fn schedule_picking_rect<T: 'static + Send + Sync>(
        &mut self,
        ctx: &RenderContext,
        picking_rect: RectInt,
        readback_identifier: GpuReadbackIdentifier,
        readback_user_data: T,
        show_debug_view: bool,
    ) -> Result<(), ViewBuilderError> {
        if self.picking_processor.is_some() {
            return Err(ViewBuilderError::PickingRectAlreadyScheduled);
        };

        let picking_processor = PickingLayerProcessor::new(
            ctx,
            &self.setup.name,
            self.setup.resolution_in_pixel.into(),
            picking_rect,
            &self.setup.frame_uniform_buffer_content,
            show_debug_view,
            readback_identifier,
            readback_user_data,
        );

        if show_debug_view {
            self.queue_draw(DebugOverlayDrawData::new(
                ctx,
                &picking_processor.picking_target,
                self.setup.resolution_in_pixel.into(),
                picking_rect,
            )?);
        }

        self.picking_processor = Some(picking_processor);

        Ok(())
    }

    /// Composites the final result of a `ViewBuilder` to a given output `RenderPass`.
    ///
    /// The bound surface(s) on the `RenderPass` are expected to be the same format as specified on `Context` creation.
    /// `screen_position` specifies where on the output pass the view is placed.
    pub fn composite(&self, ctx: &RenderContext, pass: &mut wgpu::RenderPass<'_>) {
        re_tracing::profile_function!();

        pass.set_bind_group(0, &self.setup.bind_group_0, &[]);

        self.draw_phase(
            &ctx.read_lock_renderers(),
            &ctx.gpu_resources.render_pipelines.resources(),
            DrawPhase::Compositing,
            pass,
        );
    }
}
