use smallvec::smallvec;

use crate::allocator::create_and_fill_uniform_buffer;
use crate::renderer::{
    DrawData, DrawError, DrawInstruction, DrawableCollectionViewInfo, Renderer,
    screen_triangle_vertex_shader,
};
use crate::wgpu_resources::{
    BindGroupDesc, BindGroupEntry, BindGroupLayoutDesc, GpuBindGroup, GpuBindGroupLayoutHandle,
    GpuRenderPipelineHandle, GpuTexture, PipelineLayoutDesc, RenderPipelineDesc,
};
use crate::{DrawableCollector, RenderContext, include_shader_module};

/// Supported chroma subsampling input formats.
///
/// We use `YUV`/`YCbCr`/`YPbPr` interchangeably and usually just call it `YUV`.
///
/// According to this [source](https://www.retrosix.wiki/yuv-vs-ycbcr-vs-rgb-color-space/):
/// * `YUV` is an analog signal
/// * `YCbCr` is scaled and offsetted version of YUV, used in digital signals (we denote this as "limited range YUV")
/// * `YPbPr` is the physical component cabel to transmit `YCbCr`
///
/// Actual use in the wild seems to be all over the place.
/// For instance `OpenCV` uses `YCbCr` when talking about the full range and YUV when talking about
/// limited range. [Source](https://docs.opencv.org/4.x/de/d25/imgproc_color_conversions.html):
/// > RGB <-> YCrCb JPEG [...] Y, Cr, and Cb cover the whole value range.
/// > RGB <-> YUV with subsampling [...] with resulting values Y [16, 235], U and V [16, 240] centered at 128.
///
/// For more on YUV ranges see [`YuvRange`].
///
/// Naming schema:
/// * every time a plane starts add a `_`
/// * end with `4xy` for 4:x:y subsampling.
///
/// This picture gives a great overview of how to interpret the 4:x:y naming scheme for subsampling:
/// <https://en.wikipedia.org/wiki/Chroma_subsampling#Sampling_systems_and_ratios/>
///
/// Keep indices in sync with `yuv_converter.wgsl`
#[expect(non_camel_case_types)]
#[derive(Clone, Copy, Debug)]
pub enum YuvPixelLayout {
    // ---------------------------
    // Planar formats
    // ---------------------------
    //
    /// 4:4:4 no chroma downsampling with 3 separate planes.
    /// Also known as `I444`
    ///
    /// Expects single channel data texture format.
    ///
    /// ```text
    ///            width
    ///          __________
    ///          |         |
    /// height   |    Y    |
    ///          |         |
    ///          |_________|
    ///          |         |
    /// height   |    U    |
    ///          |         |
    ///          |_________|
    ///          |         |
    /// height   |    V    |
    ///          |         |
    ///          |_________|
    /// ```
    Y_U_V444 = 0,

    /// 4:2:2 subsampling with 3 separate planes.
    /// Also known as `I422`
    ///
    /// Expects single channel data texture format.
    ///
    /// Each data texture row in U & V section contains two rows
    /// of U/V respectively, since there's a total of (width/2) * (height/2) U & V samples
    ///
    /// ```text
    ///            width
    ///          __________
    ///          |         |
    /// height   |    Y    |
    ///          |         |
    ///          |_________|
    /// height/2 |    U    |
    ///          |_________|
    /// height/2 |    V    |
    ///          |_________|
    /// ```
    Y_U_V422 = 1,

    /// 4:2:0 subsampling with 3 separate planes.
    /// Also known as `I420`
    ///
    /// Expects single channel data texture format.
    ///
    /// Each data texture row in U & V section contains two rows
    /// of U/V respectively, since there's a total of (width/2) * height U & V samples
    ///
    /// ```text
    ///            width
    ///          __________
    ///          |         |
    /// height   |    Y    |
    ///          |         |
    ///          |_________|
    /// height/4 |___◌̲U____|
    /// height/4 |___◌̲V____|
    /// ```
    Y_U_V420 = 2,

    // ---------------------------
    // Semi-planar formats
    // ---------------------------
    //
    /// 4:2:0 subsampling with a separate Y plane, followed by a UV plane.
    /// Also known as `NV12` (although `NV12` usually also implies the limited range).
    ///
    /// Expects single channel data texture format.
    ///
    /// First comes entire image in Y in one plane,
    /// followed by a plane with interleaved lines ordered as U0, V0, U1, V1, etc.
    ///
    /// ```text
    ///          width
    ///          __________
    ///          |         |
    /// height   |    Y    |
    ///          |         |
    ///          |_________|
    /// height/2 | U,V,U,… |
    ///          |_________|
    /// ```
    Y_UV420 = 100,

    // ---------------------------
    // Interleaved formats
    // ---------------------------
    //
    /// YUV 4:2:2 subsampling, single plane.
    ///
    /// Expects single channel data texture format.
    ///
    /// The order of the channels is Y0, U0, Y1, V0, all in the same plane.
    ///
    /// ```text
    ///             width * 2
    ///        __________________
    ///        |                 |
    /// height | Y0, U0, Y1, V0… |
    ///        |_________________|
    /// ```
    YUYV422 = 200,

    // ---------------------------
    // Monochrome formats
    // ---------------------------
    //
    /// 4:0:0, single plane of chroma only.
    /// Also known as I400
    ///
    /// Expects single channel data texture format.
    ///
    /// Note that we still convert this to RGBA, for convenience.
    ///
    /// ```text
    ///             width
    ///          __________
    ///          |         |
    /// height   |    Y    |
    ///          |         |
    ///          |_________|
    /// ```
    Y400 = 300,
}

impl std::fmt::Display for YuvPixelLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Y_U_V444 => write!(f, "4:4:4 (planar)"),
            Self::Y_U_V422 => write!(f, "4:2:2 (planar)"),
            Self::Y_U_V420 => write!(f, "4:2:0 (planar)"),
            Self::Y_UV420 => write!(f, "4:2:0 (semi-planar)"),
            Self::YUYV422 => write!(f, "4:2:2 (interleaved"),
            Self::Y400 => write!(f, "4:0:0"),
        }
    }
}

/// Yuv matrix coefficients that determine how a YUV image is meant to be converted to RGB.
///
/// A rigorious definition of the yuv conversion matrix would additionally require to define
/// the transfer characteristics & color primaries of the resulting RGB space.
///
/// However, at this point we generally assume that no further processing is needed after the transform.
/// This is acceptable for most non-HDR content because of the following properties of `Bt709`/`Bt601`/ sRGB:
/// * Bt709 & sRGB primaries are practically identical
/// * Bt601 PAL & Bt709 color primaries are the same (with some slight differences for Bt709 NTSC)
/// * Bt709 & sRGB transfer function are almost identical (and the difference is widely ignored)
///
/// (sources: <https://en.wikipedia.org/wiki/Rec._709>, <https://en.wikipedia.org/wiki/Rec._601>)
/// …which means for the moment we pretty much only care about the (actually quite) different YUV conversion matrices!
#[derive(Clone, Copy, Debug)]
pub enum YuvMatrixCoefficients {
    /// Identity matrix, interpret YUV as GBR.
    Identity = 0,

    /// BT.601 (aka. SDTV, aka. Rec.601)
    ///
    /// Wiki: <https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.601_conversion/>
    Bt601 = 1,

    /// BT.709 (aka. HDTV, aka. Rec.709)
    ///
    /// Wiki: <https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.709_conversion/>
    ///
    /// These are the same primaries we usually assume and use for all our rendering
    /// since they are the same primaries used by sRGB.
    /// <https://en.wikipedia.org/wiki/Rec._709#Relationship_to_sRGB/>
    /// The OETF/EOTF function (<https://en.wikipedia.org/wiki/Transfer_functions_in_imaging>) is different,
    /// but for all other purposes they are the same.
    /// (The only reason for us to convert to optical units ("linear" instead of "gamma") is for
    /// lighting & tonemapping where we typically start out with an sRGB image!)
    Bt709 = 2,
    //
    // Not yet supported. These vary a lot more from the other two!
    //
    // /// BT.2020 (aka. PQ, aka. Rec.2020)
    // ///
    // /// Wiki: <https://en.wikipedia.org/wiki/YCbCr#ITU-R_BT.2020_conversion/>
    // BT2020_ConstantLuminance,
    // BT2020_NonConstantLuminance,
}

impl std::fmt::Display for YuvMatrixCoefficients {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity => write!(f, "identity"),
            Self::Bt601 => write!(f, "BT.601"),
            Self::Bt709 => write!(f, "BT.709"),
        }
    }
}

/// Expected range of YUV values.
///
/// Keep indices in sync with `yuv_converter.wgsl`
#[derive(Clone, Copy, Debug, Default)]
pub enum YuvRange {
    /// Use limited range YUV, i.e. for 8bit data, Y is valid in [16, 235] and U/V [16, 240].
    ///
    /// This is by far the more common YUV range.
    // TODO(andreas): What about higher bit ranges?
    // This range says https://www.reddit.com/r/ffmpeg/comments/uiugfc/comment/i7f4wyp/
    // 64-940 for Y and 64-960 for chroma.
    #[default]
    Limited = 0,

    /// Use full range YUV with all components ranging from 0 to 255 for 8bit or higher otherwise.
    Full = 1,
}

impl std::fmt::Display for YuvRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Limited => write!(f, "limited"),
            Self::Full => write!(f, "full"),
        }
    }
}

impl YuvPixelLayout {
    /// Given the dimensions of the output picture, what are the expected dimensions of the input data texture.
    pub fn data_texture_width_height(&self, [decoded_width, decoded_height]: [u32; 2]) -> [u32; 2] {
        match self {
            Self::Y_U_V444 => [decoded_width, decoded_height * 3],
            Self::Y_U_V422 => [decoded_width, decoded_height * 2],
            Self::Y_U_V420 | Self::Y_UV420 => [decoded_width, decoded_height + decoded_height / 2],
            Self::YUYV422 => [decoded_width * 2, decoded_height],
            Self::Y400 => [decoded_width, decoded_height],
        }
    }

    /// What format the input data texture is expected to be in.
    pub fn data_texture_format(&self) -> wgpu::TextureFormat {
        // TODO(andreas): How to deal with higher precision formats here?
        //
        // Our shader currently works with 8 bit integer formats here since while
        // _technically_ YUV formats have nothing to do with concrete bit depth,
        // practically there's underlying expectation for 8 bits per channel
        // at least as long as the data is Bt.709 or Bt.601.
        // In other words: The conversions implementations we have today expect 0-255 as the value range.

        #[expect(clippy::match_same_arms)]
        match self {
            // Only thing that makes sense for 8 bit planar data is the R8Uint format.
            Self::Y_U_V444 | Self::Y_U_V422 | Self::Y_U_V420 => wgpu::TextureFormat::R8Uint,

            // Same for planar
            Self::Y_UV420 => wgpu::TextureFormat::R8Uint,

            // Interleaved have opportunities here!
            // TODO(andreas): Why not use [`wgpu::TextureFormat::Rg8Uint`] here?
            Self::YUYV422 => wgpu::TextureFormat::R8Uint,

            // Monochrome have only one channel anyways.
            Self::Y400 => wgpu::TextureFormat::R8Uint,
        }
    }

    /// Size of the buffer needed to create the data texture, i.e. the raw input data.
    pub fn num_data_buffer_bytes(&self, decoded_width: [u32; 2]) -> usize {
        let data_texture_width_height = self.data_texture_width_height(decoded_width);
        let data_texture_format = self.data_texture_format();

        (data_texture_format
            .block_copy_size(None)
            .expect("data texture formats are expected to be trivial")
            * data_texture_width_height[0]
            * data_texture_width_height[1]) as usize
    }
}

mod gpu_data {
    use crate::wgpu_buffer_types;

    #[repr(C)]
    #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    pub struct UniformBuffer {
        /// Uses [`super::YuvPixelLayout`].
        pub yuv_layout: u32,

        /// Uses [`super::YuvMatrixCoefficients`].
        pub yuv_matrix_coefficients: u32,

        pub target_texture_size: [u32; 2],

        /// Uses [`super::YuvRange`].
        pub yuv_range: wgpu_buffer_types::U32RowPadded,

        pub _end_padding: [wgpu_buffer_types::PaddingRow; 16 - 2],
    }
}

/// A work item for the subsampling converter.
pub struct YuvFormatConversionTask {
    bind_group: GpuBindGroup,
    target_texture: GpuTexture,
}

impl DrawData for YuvFormatConversionTask {
    type Renderer = YuvFormatConverter;

    fn collect_drawables(
        &self,
        _view_info: &DrawableCollectionViewInfo,
        _collector: &mut DrawableCollector<'_>,
    ) {
        // Doesn't participate in regular rendering.\
        // TODO(andreas): Maybe this shouldn't miss-use the `DrawData`/`Renderer` interface?
    }
}

impl YuvFormatConversionTask {
    /// Format that a target texture must have in order to be used as output of this converter.
    ///
    /// sRGB encoded 8 bit texture.
    ///
    /// Not using [`wgpu::TextureFormat::Rgba8UnormSrgb`] since consumers typically consume this
    /// texture with software EOTF ("to linear") for more flexibility.
    pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

    /// Usage flags that a target texture must have in order to be used as output of this converter.
    pub const REQUIRED_TARGET_TEXTURE_USAGE_FLAGS: wgpu::TextureUsages =
        wgpu::TextureUsages::RENDER_ATTACHMENT;

    /// Creates a new conversion task that can be used with [`YuvFormatConverter`].
    ///
    /// Does *not* validate that the input data has the expected format,
    /// see methods of [`YuvPixelLayout`] for details.
    pub fn new(
        ctx: &RenderContext,
        yuv_layout: YuvPixelLayout,
        yuv_range: YuvRange,
        yuv_matrix_coefficients: YuvMatrixCoefficients,
        input_data: &GpuTexture,
        target_texture: &GpuTexture,
    ) -> Self {
        let target_label = target_texture.creation_desc.label.clone();
        let renderer = ctx.renderer::<YuvFormatConverter>();

        let uniform_buffer = create_and_fill_uniform_buffer(
            ctx,
            format!("{target_label}_conversion").into(),
            gpu_data::UniformBuffer {
                yuv_layout: yuv_layout as _,
                yuv_matrix_coefficients: yuv_matrix_coefficients as _,
                target_texture_size: [
                    target_texture.creation_desc.size.width,
                    target_texture.creation_desc.size.height,
                ],
                yuv_range: (yuv_range as u32).into(),

                _end_padding: Default::default(),
            },
        );

        let bind_group = ctx.gpu_resources.bind_groups.alloc(
            &ctx.device,
            &ctx.gpu_resources,
            &BindGroupDesc {
                label: "RectangleInstance::bind_group".into(),
                entries: smallvec![
                    uniform_buffer,
                    BindGroupEntry::DefaultTextureView(input_data.handle),
                ],
                layout: renderer.bind_group_layout,
            },
        );

        Self {
            bind_group,
            target_texture: target_texture.clone(),
        }
    }

    /// Runs the conversion from the input texture data.
    pub fn convert_input_data_to_texture(self, ctx: &RenderContext) -> Result<(), DrawError> {
        // TODO(andreas): Does this have to be on the global view encoder?
        // If this ever becomes a problem we could easily schedule this to another encoder as long as
        // we guarantee that the conversion is enqueued before the resulting texture is used.
        // Given that we already have this neatly encapsulated work package this would be quite easy to do!
        let mut encoder = ctx.active_frame.before_view_builder_encoder.lock();
        let mut pass = encoder
            .get()
            .begin_render_pass(&wgpu::RenderPassDescriptor {
                label: self.target_texture.creation_desc.label.get(),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target_texture.default_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });

        ctx.renderer::<YuvFormatConverter>().draw(
            &ctx.gpu_resources.render_pipelines.resources(),
            crate::draw_phases::DrawPhase::Opaque, // Don't care about the phase.
            &mut pass,
            &[DrawInstruction {
                draw_data: &self,
                drawables: &[],
            }],
        )
    }
}

/// Converter for chroma subsampling formats.
///
/// Takes chroma subsampled data and draws to a fullscreen sRGB output texture.
/// Implemented as a [`Renderer`] in order to make use of the existing mechanisms for storing renderer data.
/// (we need some place to lazily create the render pipeline, store a handle to it and encapsulate the draw logic!)
pub struct YuvFormatConverter {
    render_pipeline: GpuRenderPipelineHandle,
    bind_group_layout: GpuBindGroupLayoutHandle,
}

impl Renderer for YuvFormatConverter {
    type RendererDrawData = YuvFormatConversionTask;

    fn create_renderer(ctx: &RenderContext) -> Self {
        let vertex_handle = screen_triangle_vertex_shader(ctx);

        let bind_group_layout = ctx.gpu_resources.bind_group_layouts.get_or_create(
            &ctx.device,
            &BindGroupLayoutDesc {
                label: "YuvFormatConverter".into(),
                entries: vec![
                    // Uniform buffer with some information.
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
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
                    // Input data texture.
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Uint,
                        },
                        count: None,
                    },
                ],
            },
        );

        let pipeline_layout = ctx.gpu_resources.pipeline_layouts.get_or_create(
            ctx,
            &PipelineLayoutDesc {
                label: "YuvFormatConverter".into(),
                // Note that this is a fairly unusual layout for us with the first entry
                // not being the globally set bind group!
                entries: vec![bind_group_layout],
            },
        );

        let shader_modules = &ctx.gpu_resources.shader_modules;
        let render_pipeline = ctx.gpu_resources.render_pipelines.get_or_create(
            ctx,
            &RenderPipelineDesc {
                label: "TestTriangle::render_pipeline".into(),
                pipeline_layout,
                vertex_entrypoint: "main".into(),
                vertex_handle,
                fragment_entrypoint: "fs_main".into(),
                fragment_handle: shader_modules.get_or_create(
                    ctx,
                    &include_shader_module!("../../shader/conversions/yuv_converter.wgsl"),
                ),
                vertex_buffers: smallvec![],
                render_targets: smallvec![Some(YuvFormatConversionTask::OUTPUT_FORMAT.into())],
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
            },
        );

        Self {
            render_pipeline,
            bind_group_layout,
        }
    }

    fn draw(
        &self,
        render_pipelines: &crate::wgpu_resources::GpuRenderPipelinePoolAccessor<'_>,
        _phase: crate::draw_phases::DrawPhase,
        pass: &mut wgpu::RenderPass<'_>,
        draw_instructions: &[DrawInstruction<'_, Self::RendererDrawData>],
    ) -> Result<(), DrawError> {
        let pipeline = render_pipelines.get(self.render_pipeline)?;

        pass.set_pipeline(pipeline);

        for DrawInstruction { draw_data, .. } in draw_instructions {
            pass.set_bind_group(0, &draw_data.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        Ok(())
    }
}
