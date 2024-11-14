#![allow(dead_code, unused_variables, clippy::unnecessary_wraps)]

use std::sync::Arc;

use re_video::{decode::FrameContent, Chunk, Frame, Time};

use parking_lot::Mutex;

use crate::{
    resource_managers::SourceImageDataFormat,
    video::{
        player::{TimedDecodingError, VideoTexture},
        VideoPlayerError,
    },
    wgpu_resources::GpuTexture,
    RenderContext,
};

#[derive(Default)]
struct DecoderOutput {
    frames: Vec<Frame>,

    /// Set on error; reset on success.
    error: Option<TimedDecodingError>,
}

/// Internal implementation detail of the [`super::player::VideoPlayer`].
// TODO(andreas): Meld this into `super::player::VideoPlayer`.
pub struct VideoChunkDecoder {
    decoder: Box<dyn re_video::decode::AsyncDecoder>,
    decoder_output: Arc<Mutex<DecoderOutput>>,
}

impl VideoChunkDecoder {
    pub fn new(
        debug_name: String,
        make_decoder: impl FnOnce(
            Box<dyn Fn(re_video::decode::Result<Frame>) + Send + Sync>,
        )
            -> re_video::decode::Result<Box<dyn re_video::decode::AsyncDecoder>>,
    ) -> Result<Self, VideoPlayerError> {
        re_tracing::profile_function!();

        let decoder_output = Arc::new(Mutex::new(DecoderOutput::default()));

        let on_output = {
            let decoder_output = decoder_output.clone();
            move |frame: re_video::decode::Result<Frame>| match frame {
                Ok(frame) => {
                    re_log::trace!(
                        "Decoded frame at PTS {:?}",
                        frame.info.presentation_timestamp
                    );
                    let mut output = decoder_output.lock();
                    output.frames.push(frame);
                    output.error = None; // We successfully decoded a frame, reset the error state.
                }
                Err(err) => {
                    // Many of the errors we get from a decoder are recoverable.
                    // They may be very frequent, but it's still useful to see them in the debug log for troubleshooting.
                    re_log::debug_once!("Error during decoding of {debug_name}: {err}");

                    let err = VideoPlayerError::Decoding(err);
                    let mut output = decoder_output.lock();
                    if let Some(error) = &mut output.error {
                        error.latest_error = err;
                    } else {
                        output.error = Some(TimedDecodingError::new(err));
                    }
                }
            }
        };

        let decoder = make_decoder(Box::new(on_output))?;

        Ok(Self {
            decoder,
            decoder_output,
        })
    }

    /// Start decoding the given chunk.
    pub fn decode(&mut self, chunk: Chunk) -> Result<(), VideoPlayerError> {
        self.decoder.submit_chunk(chunk)?;
        Ok(())
    }

    /// Called after submitting the last chunk.
    ///
    /// Should flush all pending frames.
    pub fn end_of_video(&mut self) -> Result<(), VideoPlayerError> {
        self.decoder.end_of_video()?;
        Ok(())
    }

    /// Get the latest decoded frame at the given time
    /// and copy it to the given texture.
    ///
    /// Drop all earlier frames to save memory.
    ///
    /// Returns [`VideoPlayerError::EmptyBuffer`] if the internal buffer is empty,
    /// which it is just after startup or after a call to [`Self::reset`].
    pub fn update_video_texture(
        &mut self,
        render_ctx: &RenderContext,
        video_texture: &mut VideoTexture,
        presentation_timestamp: Time,
    ) -> Result<(), VideoPlayerError> {
        let mut decoder_output = self.decoder_output.lock();
        let frames = &mut decoder_output.frames;

        let Some(frame_idx) = re_video::demux::latest_at_idx(
            frames,
            |frame| frame.info.presentation_timestamp,
            &presentation_timestamp,
        ) else {
            return Err(VideoPlayerError::EmptyBuffer);
        };

        // drain up-to (but not including) the frame idx, clearing out any frames
        // before it. this lets the video decoder output more frames.
        drop(frames.drain(0..frame_idx));

        // after draining all old frames, the next frame will be at index 0
        let frame_idx = 0;
        let frame = &frames[frame_idx];

        let frame_time_range = frame.info.presentation_time_range();

        let is_up_to_date = video_texture
            .frame_info
            .as_ref()
            .is_some_and(|info| info.presentation_time_range() == frame_time_range);

        if frame_time_range.contains(&presentation_timestamp) && !is_up_to_date {
            #[cfg(target_arch = "wasm32")]
            {
                video_texture.source_pixel_format = copy_web_video_frame_to_texture(
                    render_ctx,
                    &frame.content,
                    &video_texture.texture,
                )?;
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                video_texture.source_pixel_format = copy_native_video_frame_to_texture(
                    render_ctx,
                    &frame.content,
                    &video_texture.texture,
                )?;
            }

            video_texture.frame_info = Some(frame.info.clone());
        }

        Ok(())
    }

    /// Reset the video decoder and discard all frames.
    pub fn reset(&mut self) -> Result<(), VideoPlayerError> {
        self.decoder.reset()?;

        let mut decoder_output = self.decoder_output.lock();
        decoder_output.error = None;
        decoder_output.frames.clear();

        Ok(())
    }

    /// Return and clear the latest error that happened during decoding.
    pub fn take_error(&mut self) -> Option<TimedDecodingError> {
        self.decoder_output.lock().error.take()
    }
}

#[cfg(target_arch = "wasm32")]
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

    let source = {
        // TODO(jan): The wgpu version we're using doesn't support `VideoFrame` yet.
        // This got fixed in https://github.com/gfx-rs/wgpu/pull/6170 but hasn't shipped yet.
        // So instead, we just pretend this is a `HtmlVideoElement` instead.
        // SAFETY: Depends on the fact that `wgpu` passes the object through as-is,
        // and doesn't actually inspect it in any way. The browser then does its own
        // typecheck that doesn't care what kind of image source wgpu gave it.
        #[allow(unsafe_code)]
        let frame = unsafe {
            std::mem::transmute::<web_sys::VideoFrame, web_sys::HtmlVideoElement>(
                frame.clone().expect("Failed to clone the video frame"),
            )
        };
        // Fake width & height to work around wgpu validating this as if it was a `HtmlVideoElement`.
        // Since it thinks this is a `HtmlVideoElement`, it will want to call `videoWidth` and `videoHeight`
        // on it to validate the size.
        // We simply redirect `displayWidth`/`displayHeight` to `videoWidth`/`videoHeight` to make it work!
        let display_width = js_sys::Reflect::get(&frame, &"displayWidth".into())
            .expect("Failed to get displayWidth property from VideoFrame.");
        js_sys::Reflect::set(&frame, &"videoWidth".into(), &display_width)
            .expect("Failed to set videoWidth property.");
        let display_height = js_sys::Reflect::get(&frame, &"displayHeight".into())
            .expect("Failed to get displayHeight property from VideoFrame.");
        js_sys::Reflect::set(&frame, &"videoHeight".into(), &display_height)
            .expect("Failed to set videoHeight property.");

        wgpu_types::ImageCopyExternalImage {
            source: wgpu_types::ExternalImageSource::HTMLVideoElement(frame),
            origin: wgpu_types::Origin2d { x: 0, y: 0 },
            flip_y: false,
        }
    };
    let dest = wgpu::ImageCopyTextureTagged {
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
        transfer_image_data_to_texture, ImageDataDesc, SourceImageDataFormat,
        YuvMatrixCoefficients, YuvPixelLayout, YuvRange,
    };

    let format = match frame.format {
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
        re_video::PixelFormat::Rgba8Unorm | re_video::PixelFormat::Yuv { .. } => {
            wgpu::TextureFormat::Rgba8Unorm
        }
    };

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
                re_video::decode::YuvPixelLayout::Y_U_V444 => YuvPixelLayout::Y_U_V444,
                re_video::decode::YuvPixelLayout::Y_U_V422 => YuvPixelLayout::Y_U_V422,
                re_video::decode::YuvPixelLayout::Y_U_V420 => YuvPixelLayout::Y_U_V420,
                re_video::decode::YuvPixelLayout::Y400 => YuvPixelLayout::Y400,
            },
            coefficients: match coefficients {
                re_video::decode::YuvMatrixCoefficients::Identity => {
                    YuvMatrixCoefficients::Identity
                }
                re_video::decode::YuvMatrixCoefficients::Bt601 => YuvMatrixCoefficients::Bt601,
                re_video::decode::YuvMatrixCoefficients::Bt709 => YuvMatrixCoefficients::Bt709,
            },
            range: match range {
                re_video::decode::YuvRange::Limited => YuvRange::Limited,
                re_video::decode::YuvRange::Full => YuvRange::Full,
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
        },
        target_texture,
    )?;

    Ok(format)
}
