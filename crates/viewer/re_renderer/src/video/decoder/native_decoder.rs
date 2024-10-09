#![allow(dead_code, unused_variables, clippy::unnecessary_wraps)]

use std::sync::Arc;

use re_video::{Chunk, Frame, Time};

use parking_lot::Mutex;

use crate::{
    resource_managers::{
        transfer_image_data_to_texture, ColorPrimaries, ImageDataDesc, SourceImageDataFormat,
        YuvPixelLayout, YuvRange,
    },
    video::DecodingError,
    wgpu_resources::GpuTexture,
    RenderContext,
};

use super::{latest_at_idx, TimedDecodingError, VideoChunkDecoder, VideoTexture};

#[derive(Default)]
struct DecoderOutput {
    frames: Vec<Frame>,

    /// Set on error; reset on success.
    error: Option<TimedDecodingError>,
}

/// Native video decoder
pub struct NativeDecoder {
    decoder: re_video::decode::AsyncDecoder,
    decoder_output: Arc<Mutex<DecoderOutput>>,
}

impl NativeDecoder {
    pub fn new(
        debug_name: String,
        sync_decoder: Box<dyn re_video::decode::SyncDecoder + Send>,
    ) -> Result<Self, DecodingError> {
        re_tracing::profile_function!();

        let decoder_output = Arc::new(Mutex::new(DecoderOutput::default()));

        let on_output = {
            let decoder_output = decoder_output.clone();
            let debug_name = debug_name.clone();
            move |frame: re_video::decode::Result<Frame>| match frame {
                Ok(frame) => {
                    re_log::trace!("Decoded frame at {:?}", frame.timestamp);
                    let mut output = decoder_output.lock();
                    output.frames.push(frame);
                    output.error = None; // We successfully decoded a frame, reset the error state.
                }
                Err(err) => {
                    re_log::warn_once!("Error during decoding of {debug_name}: {err}");
                    let err = DecodingError::Decoding(err.to_string());
                    let mut output = decoder_output.lock();
                    if let Some(error) = &mut output.error {
                        error.latest_error = err;
                    } else {
                        output.error = Some(TimedDecodingError::new(err));
                    }
                }
            }
        };

        let decoder = re_video::decode::AsyncDecoder::new(debug_name, sync_decoder, on_output);

        Ok(Self {
            decoder,
            decoder_output,
        })
    }
}

impl VideoChunkDecoder for NativeDecoder {
    /// Start decoding the given chunk.
    fn decode(&mut self, chunk: Chunk, _is_keyframe: bool) -> Result<(), DecodingError> {
        self.decoder.decode(chunk);
        Ok(())
    }

    fn update_video_texture(
        &mut self,
        render_ctx: &RenderContext,
        video_texture: &mut VideoTexture,
        presentation_timestamp: Time,
    ) -> Result<(), DecodingError> {
        let mut decoder_output = self.decoder_output.lock();
        let frames = &mut decoder_output.frames;

        let Some(frame_idx) =
            latest_at_idx(frames, |frame| frame.timestamp, &presentation_timestamp)
        else {
            return Err(DecodingError::EmptyBuffer);
        };

        // drain up-to (but not including) the frame idx, clearing out any frames
        // before it. this lets the video decoder output more frames.
        drop(frames.drain(0..frame_idx));

        // after draining all old frames, the next frame will be at index 0
        let frame_idx = 0;
        let frame = &frames[frame_idx];

        let frame_time_range = frame.timestamp..frame.timestamp + frame.duration;

        if frame_time_range.contains(&presentation_timestamp)
            && video_texture.time_range != frame_time_range
        {
            copy_video_frame_to_texture(render_ctx, frame, &video_texture.texture)?;
            video_texture.time_range = frame_time_range;
        }

        Ok(())
    }

    /// Reset the video decoder and discard all frames.
    fn reset(&mut self) -> Result<(), DecodingError> {
        self.decoder.reset();

        let mut decoder_output = self.decoder_output.lock();
        decoder_output.error = None;
        decoder_output.frames.clear();

        Ok(())
    }

    /// Return and clear the latest error that happened during decoding.
    fn take_error(&mut self) -> Option<TimedDecodingError> {
        self.decoder_output.lock().error.take()
    }
}

fn copy_video_frame_to_texture(
    ctx: &RenderContext,
    frame: &Frame,
    target_texture: &GpuTexture,
) -> Result<(), DecodingError> {
    let format = match frame.format {
        re_video::PixelFormat::Rgb8Unorm => {
            // TODO(andreas): `ImageDataDesc` should have RGB handling!
            return copy_video_frame_to_texture(
                ctx,
                &Frame {
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
        re_video::PixelFormat::Rgb8Unorm | re_video::PixelFormat::Rgba8Unorm => {
            SourceImageDataFormat::WgpuCompatible(wgpu::TextureFormat::Rgba8Unorm)
        }
        re_video::PixelFormat::Yuv {
            layout,
            range,
            primaries,
        } => SourceImageDataFormat::Yuv {
            layout: match layout {
                re_video::decode::YuvPixelLayout::Y_U_V444 => YuvPixelLayout::Y_U_V444,
                re_video::decode::YuvPixelLayout::Y_U_V422 => YuvPixelLayout::Y_U_V422,
                re_video::decode::YuvPixelLayout::Y_U_V420 => YuvPixelLayout::Y_U_V420,
                re_video::decode::YuvPixelLayout::Y400 => YuvPixelLayout::Y400,
            },
            primaries: match primaries {
                re_video::decode::ColorPrimaries::Bt601 => ColorPrimaries::Bt601,
                re_video::decode::ColorPrimaries::Bt709 => ColorPrimaries::Bt709,
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

    Ok(())
}
