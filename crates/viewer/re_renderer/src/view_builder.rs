use std::sync::Arc;

use parking_lot::RwLock;

use crate::allocator::{GpuReadbackIdentifier, create_and_fill_uniform_buffer};
use crate::context::RenderContext;
use crate::draw_phases::{
    DrawPhase, OutlineConfig, OutlineMaskProcessor, PickingLayerError, PickingLayerProcessor,
    ScreenshotProcessor,
};
use crate::global_bindings::FrameUniformBuffer;
use crate::queueable_draw_data::QueueableDrawData;
use crate::renderer::{CompositorDrawData, DebugOverlayDrawData, DrawableCollectionViewInfo};
use crate::transform::RectTransform;
use crate::wgpu_resources::{GpuBindGroup, GpuTexture, PoolError, TextureDesc};
use crate::{DebugLabel, DrawPhaseManager, MsaaMode, RectInt, RenderConfig, Rgba};

#[derive(thiserror::Error, Debug)]
pub enum ViewBuilderError {
    #[error("Screenshot was already scheduled.")]
    ScreenshotAlreadyScheduled,

    #[error(transparent)]
    InvalidDebugOverlay(#[from] crate::renderer::DebugOverlayError),
}

/// The highest level rendering block in `re_renderer`.
/// Used to build up/collect various resources and then send them off for rendering of a single view.
pub struct ViewBuilder {
    setup: ViewTargetSetup,
    draw_phase_manager: DrawPhaseManager,

    // TODO(andreas): Consider making "render processors" a "thing" by establishing a form of hardcoded/limited-flexibility render-graph
    outline_mask_processor: Option<OutlineMaskProcessor>,
    screenshot_processor: Option<ScreenshotProcessor>,
    picking_processor: Option<PickingLayerProcessor>,
}

struct ViewTargetSetup {
    name: DebugLabel,

    camera_position: glam::Vec3A,

    bind_group_0: GpuBindGroup,
    main_target_msaa: GpuTexture,

    /// The main target with MSAA resolved.
    /// If MSAA is disabled, this is the same as `main_target_msaa`.
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

/// Aim for beauty or determinism?
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RenderMode {
    /// Default render mode
    #[default]
    Beautiful,

    /// Try to produce consistent results across different GPUs and drivers.
    ///
    /// Used for more consistent snapshot tests.
    Deterministic,
}

/// Basic configuration for a target view.
#[derive(Debug)]
pub struct TargetConfiguration {
    pub name: DebugLabel,

    /// Aim for beauty or determinism?
    pub render_mode: RenderMode,

    /// The viewport resolution in physical pixels.
    pub resolution_in_pixel: [u32; 2],
    pub view_from_world: macaw::IsoTransform,
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

    /// Configuration for the picking layer if any.
    ///
    /// If this is `None`, no picking layer will be created.
    /// For details see [`ViewPickingConfiguration`].
    pub picking_config: Option<ViewPickingConfiguration>,
}

impl Default for TargetConfiguration {
    fn default() -> Self {
        Self {
            name: "default view".into(),
            render_mode: RenderMode::Beautiful,
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
            picking_config: None,
        }
    }
}

/// Configures the readback of a rectangle from the picking layer.
///
/// The picking result will still be valid if the rectangle is partially or fully outside of bounds.
/// Areas that are not overlapping with the primary target will be filled as-if the view's target was bigger,
/// i.e. all values are valid picking IDs, it is up to the user to discard anything that is out of bounds.
///
/// Data from the picking rect needs to be retrieved via [`crate::PickingLayerProcessor::readback_result`].
/// To do so, you need to pass the exact same `identifier` and type of user data.
///
/// Received data that isn't retrieved for more than a frame will be automatically discarded.
#[derive(Debug)]
pub struct ViewPickingConfiguration {
    /// The rectangle to read back from the picking layer.
    pub picking_rect: RectInt,

    /// Identifier to be passed to the readback result.
    pub readback_identifier: GpuReadbackIdentifier,

    /// Whether to draw a debug view of the picking layer when compositing the final view.
    pub show_debug_view: bool,
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

    /// Default multisample state that any [`wgpu::RenderPipeline`] drawing to the main target needs to use.
    ///
    /// In rare cases, pipelines may want to enable alpha to coverage and/or sample masks.
    pub fn main_target_default_msaa_state(
        config: &RenderConfig,
        need_alpha_to_coverage: bool,
    ) -> wgpu::MultisampleState {
        let alpha_to_coverage_enabled = need_alpha_to_coverage && config.msaa_mode != MsaaMode::Off;

        wgpu::MultisampleState {
            count: config.msaa_mode.sample_count(),
            mask: !0,
            alpha_to_coverage_enabled,
        }
    }

    /// Default value for clearing depth buffer to infinity.
    ///
    /// 0.0 == far since we're using reverse-z.
    pub const DEFAULT_DEPTH_CLEAR: wgpu::LoadOp<f32> = wgpu::LoadOp::Clear(0.0);

    /// Default depth state for enabled depth write & read.
    pub const MAIN_TARGET_DEFAULT_DEPTH_STATE: wgpu::DepthStencilState = wgpu::DepthStencilState {
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
    };

    /// Default depth state for disabled depth write & read.
    pub const MAIN_TARGET_DEFAULT_DEPTH_STATE_NO_WRITE: wgpu::DepthStencilState =
        wgpu::DepthStencilState {
            depth_write_enabled: false,
            ..Self::MAIN_TARGET_DEFAULT_DEPTH_STATE
        };

    pub fn new(ctx: &RenderContext, config: TargetConfiguration) -> Result<Self, ViewBuilderError> {
        re_tracing::profile_function!();

        // Can't handle 0 size resolution since this would imply creating zero sized textures.
        assert_ne!(config.resolution_in_pixel[0], 0);
        assert_ne!(config.resolution_in_pixel[1], 0);

        let render_cfg = ctx.render_config();
        let msaa_enabled = render_cfg.msaa_mode != MsaaMode::Off;
        let size = wgpu::Extent3d {
            width: config.resolution_in_pixel[0],
            height: config.resolution_in_pixel[1],
            depth_or_array_layers: 1,
        };

        // TODO(andreas): Should tonemapping preferences go here as well? Likely!
        let main_target_msaa = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: format!("{:?} - main target", config.name).into(),
                size,
                mip_level_count: 1,
                sample_count: render_cfg.msaa_mode.sample_count(),
                dimension: wgpu::TextureDimension::D2,
                format: Self::MAIN_TARGET_COLOR_FORMAT,
                usage: if msaa_enabled {
                    // If MSAA is enabled, we don't read this texture ourselves as it is only used for resolve.
                    wgpu::TextureUsages::RENDER_ATTACHMENT
                } else {
                    wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING
                },
            },
        );

        // Like hdr_render_target, but with MSAA resolved.
        // We only need to distinguish this if we're using MSAA.
        let main_target_resolved = if msaa_enabled {
            ctx.gpu_resources.textures.alloc(
                &ctx.device,
                &TextureDesc {
                    label: format!("{:?} - main target resolved", config.name).into(),
                    size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: Self::MAIN_TARGET_COLOR_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                },
            )
        } else {
            main_target_msaa.clone()
        };

        let depth_buffer = ctx.gpu_resources.textures.alloc(
            &ctx.device,
            &TextureDesc {
                label: format!("{:?} - depth buffer", config.name).into(),
                size,
                mip_level_count: 1,
                sample_count: render_cfg.msaa_mode.sample_count(),
                dimension: wgpu::TextureDimension::D2,
                format: Self::MAIN_TARGET_DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
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
        }

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
            pixel_world_size_from_camera_distance,
            pixels_per_point: config.pixels_per_point,
            tan_half_fov,
            device_tier: ctx.device_caps().tier as u32,
            deterministic_rendering: match config.render_mode {
                RenderMode::Beautiful => 0,
                RenderMode::Deterministic => 1,
            },
            framebuffer_resolution: glam::vec2(
                config.resolution_in_pixel[0] as _,
                config.resolution_in_pixel[1] as _,
            )
            .into(),
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

        let mut debug_overlays: Vec<QueueableDrawData> = Vec::new();

        let outline_mask_processor = config.outline_config.as_ref().map(|outline_config| {
            OutlineMaskProcessor::new(
                ctx,
                outline_config,
                &config.name,
                config.resolution_in_pixel,
            )
        });
        let picking_processor = if let Some(picking_config) = config.picking_config {
            let picking_processor = PickingLayerProcessor::new(
                ctx,
                &config.name,
                config.resolution_in_pixel.into(),
                picking_config.picking_rect,
                &frame_uniform_buffer_content,
                picking_config.show_debug_view,
                picking_config.readback_identifier,
            );

            if picking_config.show_debug_view {
                debug_overlays.push(
                    DebugOverlayDrawData::new(
                        ctx,
                        &picking_processor.picking_target,
                        config.resolution_in_pixel.into(),
                        picking_config.picking_rect,
                    )?
                    .into(),
                );
            }

            Some(picking_processor)
        } else {
            None
        };

        let active_draw_phases = {
            let mut active_draw_phases = DrawPhase::Opaque
                | DrawPhase::Background
                | DrawPhase::Transparent
                | DrawPhase::Compositing;
            if config.outline_config.is_some() {
                active_draw_phases |= DrawPhase::OutlineMask;
            }
            if picking_processor.is_some() {
                active_draw_phases |= DrawPhase::PickingLayer;
            }
            // TODO(andreas): should not always be active.
            // TODO(andreas): The fact that this is a draw phase is actually a bit dubious.
            //if screenshot_processor.is_some() {
            active_draw_phases |= DrawPhase::CompositingScreenshot;
            //}

            active_draw_phases
        };

        let draw_phase_manager = DrawPhaseManager::new(active_draw_phases);

        let setup = ViewTargetSetup {
            name: config.name,
            camera_position: camera_position.into(),
            bind_group_0,
            main_target_msaa,
            main_target_resolved,
            depth_buffer,
            resolution_in_pixel: config.resolution_in_pixel,
        };

        ctx.active_frame
            .num_view_builders_created
            .fetch_add(1, std::sync::atomic::Ordering::Release);

        let mut view_builder = Self {
            setup,
            draw_phase_manager,
            outline_mask_processor,
            screenshot_processor: Default::default(),
            picking_processor,
        };

        view_builder.queue_draw(
            ctx,
            CompositorDrawData::new(
                ctx,
                &view_builder.setup.main_target_resolved,
                view_builder
                    .outline_mask_processor
                    .as_ref()
                    .map(|p| p.final_voronoi_texture()),
                &config.outline_config,
                config.blend_with_background,
            ),
        );

        for debug_overlay in debug_overlays {
            view_builder.queue_draw(ctx, debug_overlay);
        }

        Ok(view_builder)
    }

    /// Resolution in pixels as configured on view builder creation.
    pub fn resolution_in_pixel(&self) -> [u32; 2] {
        self.setup.resolution_in_pixel
    }

    pub fn queue_draw(
        &mut self,
        ctx: &RenderContext,
        draw_data: impl Into<QueueableDrawData>,
    ) -> &mut Self {
        let view_info = DrawableCollectionViewInfo {
            camera_world_position: self.setup.camera_position,
        };
        self.draw_phase_manager
            .add_draw_data(ctx, draw_data.into(), &view_info);
        self
    }

    /// Draws the frame as instructed to a temporary HDR target.
    pub fn draw(
        &mut self,
        ctx: &RenderContext,
        clear_color: Rgba,
    ) -> Result<wgpu::CommandBuffer, PoolError> {
        re_tracing::profile_function!();

        // Renderers and render pipelines are locked for the entirety of this method:
        // This means it's *not* possible to add renderers or pipelines while drawing is in progress!
        // Renderers can't be added anyways at this point (RendererData add their Renderer on creation),
        // so no point in taking the lock repeatedly.
        //
        // This used to be due to the lifetime association render passes had all passed in resources,
        // this restriction has been lifted by now in wgpu.
        // However, having our locking concentrated for the duration of a view draw
        // is also beneficial since it enforces the model of prepare->draw which avoids a lot of repeated
        // locking and unlocking.
        //
        // TODO(andreas): No longer having those lifetime issues with wgpu may still save us some locking though?

        let renderers = ctx.read_lock_renderers();
        let pipelines = ctx.gpu_resources.render_pipelines.resources();

        let setup = &self.setup;

        // Prepare the drawables for drawing!
        self.draw_phase_manager.sort_drawables();

        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: setup.name.clone().get(),
            });

        {
            re_tracing::profile_scope!("main target pass");

            let needs_msaa_resolve = ctx.render_config().msaa_mode != MsaaMode::Off;

            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: DebugLabel::from(format!("{} - main pass", setup.name)).get(),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &setup.main_target_msaa.default_view,
                    depth_slice: None,
                    resolve_target: needs_msaa_resolve
                        .then_some(&setup.main_target_resolved.default_view),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: clear_color.r() as f64,
                            g: clear_color.g() as f64,
                            b: clear_color.b() as f64,
                            a: clear_color.a() as f64,
                        }),
                        store: if needs_msaa_resolve {
                            // Don't care about the result, if it's going to be resolved to the resolve target.
                            // This can have be much better perf, especially on tiler gpus.
                            wgpu::StoreOp::Discard
                        } else {
                            // Otherwise, we do need the result for the next pass.
                            wgpu::StoreOp::Store
                        },
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
                self.draw_phase_manager
                    .draw(&renderers, &pipelines, phase, &mut pass);
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
                self.draw_phase_manager.draw(
                    &renderers,
                    &pipelines,
                    DrawPhase::PickingLayer,
                    &mut pass,
                );
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
                self.draw_phase_manager.draw(
                    &renderers,
                    &pipelines,
                    DrawPhase::OutlineMask,
                    &mut pass,
                );
            }
            outline_mask_processor.compute_outlines(&pipelines, &mut encoder)?;
        }

        if let Some(screenshot_processor) = &self.screenshot_processor {
            {
                let mut pass = screenshot_processor.begin_render_pass(&setup.name, &mut encoder);
                pass.set_bind_group(0, &setup.bind_group_0, &[]);
                self.draw_phase_manager.draw(
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
        }

        self.screenshot_processor = Some(ScreenshotProcessor::new(
            ctx,
            &self.setup.name,
            self.setup.resolution_in_pixel.into(),
            identifier,
            user_data,
        ));

        Ok(())
    }

    /// Composites the final result of a `ViewBuilder` to a given output `RenderPass`.
    ///
    /// The bound surface(s) on the `RenderPass` are expected to be the same format as specified on `Context` creation.
    /// `screen_position` specifies where on the output pass the view is placed.
    pub fn composite(&self, ctx: &RenderContext, pass: &mut wgpu::RenderPass<'_>) {
        re_tracing::profile_function!();

        pass.set_bind_group(0, &self.setup.bind_group_0, &[]);

        self.draw_phase_manager.draw(
            &ctx.read_lock_renderers(),
            &ctx.gpu_resources.render_pipelines.resources(),
            DrawPhase::Compositing,
            pass,
        );
    }
}
