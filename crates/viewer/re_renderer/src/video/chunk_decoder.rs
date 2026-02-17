use re_video::{Frame, FrameContent};

use super::{VideoPlayerError, VideoTexture};
use crate::RenderContext;
use crate::resource_managers::{AlphaChannelUsage, GpuTexture2D, SourceImageDataFormat};
use crate::wgpu_resources::{GpuTexture, GpuTexturePool, TextureDesc};

pub fn update_video_texture_with_frame(
    render_ctx: &RenderContext,
    target_video_texture: &mut VideoTexture,
    source_frame: &Frame,
) -> Result<(), VideoPlayerError> {
    let Frame {
        content: source_content,
        info: _,
    } = source_frame;

    // Ensure that we have a texture to copy to.
    let gpu_texture = target_video_texture.texture.get_or_insert_with(|| {
        alloc_video_frame_texture(
            &render_ctx.device,
            &render_ctx.gpu_resources.textures,
            source_content.width(),
            source_content.height(),
        )
    });

    let format = copy_frame_to_texture(render_ctx, source_content, gpu_texture)?;

    target_video_texture.source_pixel_format = format;

    Ok(())
}

fn alloc_video_frame_texture(
    device: &wgpu::Device,
    pool: &GpuTexturePool,
    width: u32,
    height: u32,
) -> GpuTexture2D {
    let Some(texture) = GpuTexture2D::new(
        pool.alloc(
            device,
            &TextureDesc {
                label: "video".into(),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                // Needs [`wgpu::TextureUsages::RENDER_ATTACHMENT`], otherwise copy of external textures will fail.
                // Adding [`wgpu::TextureUsages::COPY_SRC`] so we can read back pixels on demand.
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::RENDER_ATTACHMENT,
            },
        ),
        // Technically, there are video codecs with alpha channel, but it's super rare.
        AlphaChannelUsage::Opaque,
    ) else {
        // We set the dimension to `2D` above, so this should never happen.
        unreachable!();
    };

    texture
}

pub fn copy_frame_to_texture(
    ctx: &RenderContext,
    frame: &FrameContent,
    target_texture: &GpuTexture,
) -> Result<SourceImageDataFormat, VideoPlayerError> {
    #[cfg(target_arch = "wasm32")]
    {
        copy_web_video_frame_to_texture(ctx, frame, target_texture)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        copy_native_video_frame_to_texture(ctx, frame, target_texture)
    }
}

#[cfg(target_arch = "wasm32")]
#[expect(clippy::unnecessary_wraps)]
fn copy_web_video_frame_to_texture(
    ctx: &RenderContext,
    frame: &FrameContent,
    target_texture: &GpuTexture,
) -> Result<SourceImageDataFormat, VideoPlayerError> {
    let size = wgpu::Extent3d {
        width: frame.display_width(),
        height: frame.display_height(),
        depth_or_array_layers: 1,
    };
    let frame: &web_sys::VideoFrame = frame;
    let source = wgpu::CopyExternalImageSourceInfo {
        // Careful: `web_sys::VideoFrame` has a custom `clone` method:
        // https://developer.mozilla.org/en-US/docs/Web/API/VideoFrame/clone
        // We instead just want to clone the js value wrapped in VideoFrame!
        source: wgpu::ExternalImageSource::VideoFrame(Clone::clone(frame)),
        origin: wgpu::Origin2d { x: 0, y: 0 },
        flip_y: false,
    };
    let dest = wgpu::CopyExternalImageDestInfo {
        texture: &target_texture.texture,
        mip_level: 0,
        origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
        aspect: wgpu::TextureAspect::All,
        color_space: wgpu::PredefinedColorSpace::Srgb,
        premultiplied_alpha: false,
    };

    ctx.queue
        .copy_external_image_to_texture(&source, dest, size);

    Ok(SourceImageDataFormat::WgpuCompatible(
        target_texture.creation_desc.format,
    ))
}

#[cfg(not(target_arch = "wasm32"))]
fn copy_native_video_frame_to_texture(
    ctx: &RenderContext,
    frame: &FrameContent,
    target_texture: &GpuTexture,
) -> Result<SourceImageDataFormat, VideoPlayerError> {
    use crate::resource_managers::{
        ImageDataDesc, SourceImageDataFormat, YuvMatrixCoefficients, YuvPixelLayout, YuvRange,
        transfer_image_data_to_texture,
    };

    // Recurse above `profile_function`.
    match frame.format {
        re_video::PixelFormat::Rgb8Unorm => {
            // TODO(andreas): `ImageDataDesc` should have RGB handling!
            return copy_native_video_frame_to_texture(
                ctx,
                &FrameContent {
                    data: crate::pad_rgb_to_rgba(&frame.data, 255_u8),
                    format: re_video::PixelFormat::Rgba8Unorm,
                    ..*frame
                },
                target_texture,
            );
        }
        re_video::PixelFormat::Rgba8Unorm | re_video::PixelFormat::Yuv { .. } => {}
    }

    re_tracing::profile_function!();

    let format = match &frame.format {
        re_video::PixelFormat::Rgb8Unorm => {
            unreachable!("Handled explicitly earlier in this function");
        }

        re_video::PixelFormat::Rgba8Unorm => {
            SourceImageDataFormat::WgpuCompatible(wgpu::TextureFormat::Rgba8Unorm)
        }

        re_video::PixelFormat::Yuv {
            layout,
            range,
            coefficients,
        } => SourceImageDataFormat::Yuv {
            layout: match layout {
                re_video::YuvPixelLayout::Y_U_V444 => YuvPixelLayout::Y_U_V444,
                re_video::YuvPixelLayout::Y_U_V422 => YuvPixelLayout::Y_U_V422,
                re_video::YuvPixelLayout::Y_U_V420 => YuvPixelLayout::Y_U_V420,
                re_video::YuvPixelLayout::Y400 => YuvPixelLayout::Y400,
            },
            coefficients: match coefficients {
                re_video::YuvMatrixCoefficients::Identity => YuvMatrixCoefficients::Identity,
                re_video::YuvMatrixCoefficients::Bt601 => YuvMatrixCoefficients::Bt601,
                re_video::YuvMatrixCoefficients::Bt709 => YuvMatrixCoefficients::Bt709,
            },
            range: match range {
                re_video::YuvRange::Limited => YuvRange::Limited,
                re_video::YuvRange::Full => YuvRange::Full,
            },
        },
    };

    transfer_image_data_to_texture(
        ctx,
        ImageDataDesc {
            label: "video_texture_upload".into(),
            data: std::borrow::Cow::Borrowed(frame.data.as_slice()),
            format,
            width_height: [frame.width, frame.height],
            // Technically, there are video codecs with alpha channel, but it's super rare.
            alpha_channel_usage: AlphaChannelUsage::Opaque,
        },
        target_texture,
    )?;

    Ok(format)
}
