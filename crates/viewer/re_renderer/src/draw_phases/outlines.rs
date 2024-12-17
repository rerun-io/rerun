//! Outlines as postprocessing effect.
//!
//! This module provides the [`OutlineMaskProcessor`] which handles render passes around outlines.
//! The outlines themselves are evaluated and drawn by the main compositor.
//!
//! There are two channels (in shader code referred to as A and B) that are handled simultaneously.
//! For configuring the look of the outline refer to [`OutlineConfig`].
//! For setting outlines for an individual primitive from another [`crate::renderer::Renderer`]/[`crate::renderer::DrawData`],
//! check for [`OutlineMaskPreference`] settings on that primitive.
//!
//! How it works:
//! =============
//! The basic approach follows closely @bgolus' [blog post](https://bgolus.medium.com/the-quest-for-very-wide-outlines-ba82ed442cd9)
//! on jump-flooding based outlines.
//!
//! Quick recap & overview:
//! * Render scene into a mask texture
//! * Extract a contour from the mask texture, for each contour pixel write the position in the (to-be) voronoi texture.
//!     * in our case we extract all pixels at which the mask changes (details below)
//! * Jump-flooding iterations: For each pixel in the voronoi texture,
//!   sample the current pixel and an 8-neighborhood at a certain, for each pass decreasing, distance and write out the closest position seen so far.
//!     * This is repeated for `log2(outline_width)` iterations.
//! * During composition, extract an outline by checking the distance to the closest contour using the voronoi texture
//!
//! What makes our implementation (a little bit) special:
//! -----------------------------------------------------
//! In short: We have more complex outline relationships but do so without additional passes!
//!
//! * Different objects may have outlines between each other
//!     * This is achieved by making the mask texture a 2 channel texture, where each channel is a different 8bit object id.
//!         * object ids are arbitrary and only for the purpose of distinguishing between outlines
//!     * Since we now no longer can resolve anti-aliasing in a straight forward manner (can't blend object ids!),
//!         * This implies a custom resolve during contour extraction!
//!     * It seems to force our hand towards outlines that extend inwards:
//!         * For each channel A & B we only get a single voronoi texture (fused into one 4 channel texture),
//!           meaning that we only have a single unsigned distance to the closest contour.
//!           If we don't want to ignore objects drawn upon each other, we need to compute the distance to any contour (== pixel where object id changes).
//!         * It might be possible to mask out inner outlines during composition, but it's not clear what the exact masking rules are for this.
//! * We use two channels (A and B) for outlines, so that we can have two independent outlines (even for the same object if desired)
//!     * We do this in a single pass by using a 2 channel texture on the mask (object id A, object id B) and
//!       a 4 channel texture on the voronoi texture (xy coordinates for A, xy coordinates for B)
//!
//! More details can be found in the respective shader code.
//!

use crate::{
    allocator::create_and_fill_uniform_buffer_batch,
    config::DeviceTier,
    include_shader_module,
    renderer::screen_triangle_vertex_shader,
    view_builder::ViewBuilder,
    wgpu_resources::{
        BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
        GpuRenderPipelineHandle, GpuRenderPipelinePoolAccessor, GpuTexture, PipelineLayoutDesc,
        PoolError, RenderPipelineDesc, SamplerDesc,
    },
    DebugLabel, RenderContext,
};

use smallvec::smallvec;

/// What outline (if any) should be drawn.
///
/// Outlines have two channels (referred to as A and B).
/// Each channel can distinguish up 255 different objects, each getting their own outline.
///
/// Object index 0 is special: It is the default background of each outline channel, thus rendering with it
/// is a form of "active no outline", effectively subtracting from any outline channel.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct OutlineMaskPreference(pub Option<[u8; 2]>);

impl OutlineMaskPreference {
    pub const NONE: Self = Self(None);

    #[inline]
    pub fn some(channel_a: u8, channel_b: u8) -> Self {
        Self(Some([channel_a, channel_b]))
    }

    #[inline]
    pub fn is_some(self) -> bool {
        self.0.is_some()
    }

    #[inline]
    pub fn is_none(self) -> bool {
        self.0.is_none()
    }

    /// Uses current outline and falls back to `other` if current is `None` or has a zero on any channel.
    #[inline]
    pub fn with_fallback_to(self, other: Self) -> Self {
        if let Some([a, b]) = self.0 {
            if let Some([other_a, other_b]) = other.0 {
                Self::some(
                    if a == 0 { other_a } else { a },
                    if b == 0 { other_b } else { b },
                )
            } else {
                self
            }
        } else {
            other
        }
    }
}

#[derive(Clone, Debug)]
pub struct OutlineConfig {
    /// Outline radius for both layers in pixels. Fractional pixels are valid.
    ///
    /// Could do different radius for both layers if the need arises, but for now this simplifies things.
    pub outline_radius_pixel: f32,

    /// Premultiplied RGBA color for the first outline layer.
    pub color_layer_a: crate::Rgba,

    /// Premultiplied RGBA color for the second outline layer.
    pub color_layer_b: crate::Rgba,
}

// TODO(andreas): Is this a sort of DrawPhase implementor? Need a system for this.
pub struct OutlineMaskProcessor {
    label: DebugLabel,

    mask_texture: GpuTexture,
    mask_depth: GpuTexture,
    voronoi_textures: [GpuTexture; 2],

    bind_group_jumpflooding_init: GpuBindGroup,
    bind_group_jumpflooding_steps: Vec<GpuBindGroup>,

    render_pipeline_jumpflooding_init: GpuRenderPipelineHandle,
    render_pipeline_jumpflooding_step: GpuRenderPipelineHandle,
}

mod gpu_data {
    use crate::wgpu_buffer_types;

    /// Keep in sync with `jumpflooding_step.wgsl`
    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct JumpfloodingStepUniformBuffer {
        pub step_width: wgpu_buffer_types::U32RowPadded,

        /// All this padding hurts. `step_width` be a `PushConstant` but they are not widely supported enough!
        pub end_padding: [wgpu_buffer_types::PaddingRow; 16 - 1],
    }
}

impl OutlineMaskProcessor {
    /// Format of the outline mask target.
    ///
    /// Two channels with each 256 object ids.
    pub const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg8Uint;
    pub const MASK_DEPTH_FORMAT: wgpu::TextureFormat = ViewBuilder::MAIN_TARGET_DEPTH_FORMAT;
    pub const MASK_DEPTH_STATE: Option<wgpu::DepthStencilState> = Some(wgpu::DepthStencilState {
        format: Self::MASK_DEPTH_FORMAT,
        // Use GreaterEQUAL in order to make outlines overridable.
        // This is useful when a large batch shares a common outline, but some of the items in the batch are rendered again with different outlines.
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

    /// Holds two pairs of pixel coordinates (one for each layer).
    const VORONOI_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    /// Default MSAA state for the outline mask target.
    pub fn mask_default_msaa_state(tier: DeviceTier) -> wgpu::MultisampleState {
        wgpu::MultisampleState {
            count: Self::mask_sample_count(tier),
            mask: !0,
            alpha_to_coverage_enabled: false,
        }
    }

    /// Number of MSAA samples used for the outline mask target.
    pub fn mask_sample_count(tier: DeviceTier) -> u32 {
        if tier.support_sampling_msaa_texture() {
            // The MSAA shader variant deals with *exactly* 4 samples.
            // See `jumpflooding_step_msaa.wgsl`.
            4
        } else {
            1
        }
    }

    pub fn new(
        ctx: &RenderContext,
        config: &OutlineConfig,
        view_name: &DebugLabel,
        resolution_in_pixel: [u32; 2],
    ) -> Self {
        re_tracing::profile_function!();
        let instance_label: DebugLabel = format!("{view_name} - OutlineMaskProcessor").into();

        // ------------- Textures -------------
        let texture_pool = &ctx.gpu_resources.textures;

        let mask_sample_count = Self::mask_sample_count(ctx.device_caps().tier);
        let mask_texture_desc = crate::wgpu_resources::TextureDesc {
            label: format!("{instance_label}::mask_texture").into(),
            size: wgpu::Extent3d {
                width: resolution_in_pixel[0],
                height: resolution_in_pixel[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: mask_sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: Self::MASK_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        };
        let mask_texture = texture_pool.alloc(&ctx.device, &mask_texture_desc);

        // We have a fresh depth buffer here that we need because:
        // * We want outlines visible even if there's an object in front, so don't re-use previous
        // * Overdraw IDs correctly
        // * TODO(andreas): Make overdrawn outlines more transparent by comparing depth
        let mask_depth = texture_pool.alloc(
            &ctx.device,
            &crate::wgpu_resources::TextureDesc {
                label: format!("{instance_label}::mask_depth").into(),
                format: Self::MASK_DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                ..mask_texture_desc
            },
        );

        let voronoi_texture_desc = crate::wgpu_resources::TextureDesc {
            label: format!("{instance_label}::distance_texture").into(),
            sample_count: 1,
            format: Self::VORONOI_FORMAT,
            ..mask_texture_desc
        };
        let voronoi_textures = [
            texture_pool.alloc(&ctx.device, &voronoi_texture_desc.with_label_push("0")),
            texture_pool.alloc(&ctx.device, &voronoi_texture_desc.with_label_push("1")),
        ];

        // ------------- Bind Groups -------------

        let (bind_group_jumpflooding_init, bind_group_layout_jumpflooding_init) =
            Self::create_bind_group_jumpflooding_init(ctx, &instance_label, &mask_texture);
        let (bind_group_jumpflooding_steps, bind_group_layout_jumpflooding_step) =
            Self::create_bind_groups_for_jumpflooding_steps(
                config,
                ctx,
                &instance_label,
                &voronoi_textures,
            );

        // ------------- Render Pipelines -------------

        let screen_triangle_vertex_shader = screen_triangle_vertex_shader(ctx);
        let jumpflooding_init_shader_module = if mask_sample_count == 1 {
            include_shader_module!("../../shader/outlines/jumpflooding_init.wgsl")
        } else {
            include_shader_module!("../../shader/outlines/jumpflooding_init_msaa.wgsl")
        };
        let jumpflooding_init_desc = RenderPipelineDesc {
            label: "OutlineMaskProcessor::jumpflooding_init".into(),
            pipeline_layout: ctx.gpu_resources.pipeline_layouts.get_or_create(
                ctx,
                &PipelineLayoutDesc {
                    label: "OutlineMaskProcessor::jumpflooding_init".into(),
                    entries: vec![bind_group_layout_jumpflooding_init],
                },
            ),
            vertex_entrypoint: "main".into(),
            vertex_handle: screen_triangle_vertex_shader,
            fragment_entrypoint: "main".into(),
            fragment_handle: ctx
                .gpu_resources
                .shader_modules
                .get_or_create(ctx, &jumpflooding_init_shader_module),
            vertex_buffers: smallvec![],
            render_targets: smallvec![Some(Self::VORONOI_FORMAT.into())],
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
        };
        let render_pipeline_jumpflooding_init = ctx
            .gpu_resources
            .render_pipelines
            .get_or_create(ctx, &jumpflooding_init_desc);
        let render_pipeline_jumpflooding_step = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "OutlineMaskProcessor::jumpflooding_step".into(),
                pipeline_layout: ctx.gpu_resources.pipeline_layouts.get_or_create(
                    ctx,
                    &PipelineLayoutDesc {
                        label: "OutlineMaskProcessor::jumpflooding_step".into(),
                        entries: vec![bind_group_layout_jumpflooding_step],
                    },
                ),
                fragment_handle: ctx.gpu_resources.shader_modules.get_or_create(
                    ctx,
                    &include_shader_module!("../../shader/outlines/jumpflooding_step.wgsl"),
                ),
                ..jumpflooding_init_desc
            },
        );

        Self {
            label: instance_label,
            mask_texture,
            mask_depth,
            voronoi_textures,
            bind_group_jumpflooding_init,
            bind_group_jumpflooding_steps,
            render_pipeline_jumpflooding_init,
            render_pipeline_jumpflooding_step,
        }
    }

    pub fn final_voronoi_texture(&self) -> &GpuTexture {
        // Point to the last written voronoi texture
        // We start writing to voronoi_textures[0] and then do `num_steps` ping-pong rendering.
        // Therefore, the last texture is voronoi_textures[num_steps % 2]
        &self.voronoi_textures[self.bind_group_jumpflooding_steps.len() % 2]
    }

    pub fn start_mask_render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> wgpu::RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: DebugLabel::from(format!("{} - mask pass", self.label)).get(),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.mask_texture.default_view,
                resolve_target: None, // We're going to do a manual resolve.
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.mask_depth.default_view,
                depth_ops: Some(wgpu::Operations {
                    load: ViewBuilder::DEFAULT_DEPTH_CLEAR,
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        })
    }

    pub fn compute_outlines(
        &self,
        pipelines: &GpuRenderPipelinePoolAccessor<'_>,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<(), PoolError> {
        let ops = wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), // Clear is the closest to "don't care"
            store: wgpu::StoreOp::Store,
        };

        // Initialize the jump flooding into voronoi texture 0 by looking at the mask texture.
        {
            let mut jumpflooding_init = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: DebugLabel::from(format!("{} - jumpflooding_init", self.label)).get(),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.voronoi_textures[0].default_view,
                    resolve_target: None,
                    ops,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            let render_pipeline_init = pipelines.get(self.render_pipeline_jumpflooding_init)?;
            jumpflooding_init.set_bind_group(0, &self.bind_group_jumpflooding_init, &[]);
            jumpflooding_init.set_pipeline(render_pipeline_init);
            jumpflooding_init.draw(0..3, 0..1);
        }

        // Perform jump flooding.
        let render_pipeline_step = pipelines.get(self.render_pipeline_jumpflooding_step)?;
        for (i, bind_group) in self.bind_group_jumpflooding_steps.iter().enumerate() {
            let mut jumpflooding_step = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: DebugLabel::from(format!("{} - jumpflooding_step {i}", self.label)).get(),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    // Start with texture 1 since the init step wrote to texture 0
                    view: &self.voronoi_textures[(i + 1) % 2].default_view,
                    resolve_target: None,
                    ops,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            jumpflooding_step.set_pipeline(render_pipeline_step);
            jumpflooding_step.set_bind_group(0, bind_group, &[]);
            jumpflooding_step.draw(0..3, 0..1);
        }

        Ok(())
    }

    fn create_bind_group_jumpflooding_init(
        ctx: &RenderContext,
        instance_label: &DebugLabel,
        mask_texture: &GpuTexture,
    ) -> (GpuBindGroup, GpuBindGroupLayoutHandle) {
        let bind_group_layout_jumpflooding_init =
            ctx.gpu_resources.bind_group_layouts.get_or_create(
                &ctx.device,
                &BindGroupLayoutDesc {
                    label: "OutlineMaskProcessor::bind_group_layout_jumpflooding_init".into(),
                    entries: vec![wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Uint,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: mask_texture.texture.sample_count() > 1,
                        },
                        count: None,
                    }],
                },
            );
        (
            ctx.gpu_resources.bind_groups.alloc(
                &ctx.device,
                &ctx.gpu_resources,
                &BindGroupDesc {
                    label: format!("{instance_label}::jumpflooding_init").into(),
                    entries: smallvec![BindGroupEntry::DefaultTextureView(mask_texture.handle)],
                    layout: bind_group_layout_jumpflooding_init,
                },
            ),
            bind_group_layout_jumpflooding_init,
        )
    }

    fn create_bind_groups_for_jumpflooding_steps(
        config: &OutlineConfig,
        ctx: &RenderContext,
        instance_label: &DebugLabel,
        voronoi_textures: &[GpuTexture; 2],
    ) -> (Vec<GpuBindGroup>, GpuBindGroupLayoutHandle) {
        let bind_group_layout_jumpflooding_step =
            ctx.gpu_resources.bind_group_layouts.get_or_create(
                &ctx.device,
                &BindGroupLayoutDesc {
                    label: "OutlineMaskProcessor::bind_group_layout_jumpflooding_step".into(),
                    entries: vec![
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                // Dynamic offset would make sense here since we cycle through a bunch of these.
                                // But we need at least two bind groups anyways since we're ping-ponging between two textures,
                                // which would make this needlessly complicated.
                                has_dynamic_offset: false,
                                min_binding_size: std::num::NonZeroU64::new(std::mem::size_of::<
                                    gpu_data::JumpfloodingStepUniformBuffer,
                                >(
                                )
                                    as _),
                            },
                            count: None,
                        },
                    ],
                },
            );

        let max_step_width =
            (config.outline_radius_pixel.max(1.0).ceil() as u32).next_power_of_two();
        let num_steps = max_step_width.ilog2() + 1;
        let uniform_buffer_jumpflooding_steps_bindings = create_and_fill_uniform_buffer_batch(
            ctx,
            "jumpflooding uniformbuffer".into(),
            (0..num_steps).map(|step| gpu_data::JumpfloodingStepUniformBuffer {
                step_width: (max_step_width >> step).into(),
                end_padding: Default::default(),
            }),
        );
        let sampler = ctx.gpu_resources.samplers.get_or_create(
            &ctx.device,
            &SamplerDesc {
                label: "nearest_clamp".into(),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                ..Default::default()
            },
        );
        let uniform_buffer_jumpflooding_steps = uniform_buffer_jumpflooding_steps_bindings
            .into_iter()
            .enumerate()
            .map(|(i, uniform_buffer_binding)| {
                ctx.gpu_resources.bind_groups.alloc(
                    &ctx.device,
                    &ctx.gpu_resources,
                    &BindGroupDesc {
                        label: format!("{instance_label}::jumpflooding_steps[{i}]").into(),
                        entries: smallvec![
                            BindGroupEntry::DefaultTextureView(voronoi_textures[i % 2].handle),
                            BindGroupEntry::Sampler(sampler),
                            uniform_buffer_binding
                        ],
                        layout: bind_group_layout_jumpflooding_step,
                    },
                )
            })
            .collect();

        (
            uniform_buffer_jumpflooding_steps,
            bind_group_layout_jumpflooding_step,
        )
    }
}
