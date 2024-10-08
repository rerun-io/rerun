#![allow(dead_code, unused_variables, clippy::unnecessary_wraps)]

use std::sync::Arc;

use re_video::{Chunk, Frame, Time};

use parking_lot::Mutex;

use crate::{video::DecodingError, RenderContext};

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
            copy_video_frame_to_texture(&render_ctx.queue, frame, &video_texture.texture.texture)?;
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
    queue: &wgpu::Queue,
    frame: &Frame,
    texture: &wgpu::Texture,
) -> Result<(), DecodingError> {
    re_tracing::profile_function!();

    let size = wgpu::Extent3d {
        width: frame.width,
        height: frame.height,
        depth_or_array_layers: 1,
    };

    let format = match frame.format {
        re_video::PixelFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
    };

    let width_blocks = frame.width / format.block_dimensions().0;

    #[allow(clippy::unwrap_used)] // block_copy_size can only fail for weird compressed formats
    let block_size = format
        .block_copy_size(Some(wgpu::TextureAspect::All))
        .unwrap();

    let bytes_per_row_unaligned = width_blocks * block_size;

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &frame.data,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(bytes_per_row_unaligned),
            rows_per_image: None,
        },
        size,
    );

    Ok(())
}
