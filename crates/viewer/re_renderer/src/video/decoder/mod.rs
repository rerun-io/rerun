#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
mod native_decoder;

use std::{ops::Range, sync::Arc, time::Duration};

use web_time::Instant;

use re_video::{Chunk, Time};

use crate::{
    resource_managers::GpuTexture2D,
    wgpu_resources::{GpuTexturePool, TextureDesc},
    RenderContext,
};

use super::{DecodeHardwareAcceleration, DecodingError, VideoFrameTexture};

/// Ignore hickups lasting shorter than this.
///
/// Delaying error reports (and showing last-good images meanwhile) allows us to skip over
/// transient errors without flickering.
///
/// Same with showing a spinner: if we show it too fast, it is annoying.
const DECODING_GRACE_DELAY: Duration = Duration::from_millis(400);

#[allow(unused)] // Unused for certain build flags
#[derive(Debug)]
struct TimedDecodingError {
    time_of_first_error: Instant,
    latest_error: DecodingError,
}

impl TimedDecodingError {
    #[allow(unused)] // Unused for certain build flags
    pub fn new(latest_error: DecodingError) -> Self {
        Self {
            time_of_first_error: Instant::now(),
            latest_error,
        }
    }
}

/// A texture of a specific video frame.
struct VideoTexture {
    pub texture: GpuTexture2D,

    /// What part of the video this video frame covers.
    pub time_range: Range<Time>,
}

/// Decode video to a texture.
///
/// If you want to sample multiple points in a video simultaneously, use multiple decoders.
trait VideoChunkDecoder: 'static + Send {
    /// Start decoding the given chunk.
    fn decode(&mut self, chunk: Chunk, is_keyframe: bool) -> Result<(), DecodingError>;

    /// Get the latest decoded frame at the given time
    /// and copy it to the given texture.
    ///
    /// Drop all earlier frames to save memory.
    ///
    /// Returns [`DecodingError::EmptyBuffer`] if the internal buffer is empty,
    /// which it is just after startup or after a call to [`Self::reset`].
    fn update_video_texture(
        &mut self,
        render_ctx: &RenderContext,
        video_texture: &mut VideoTexture,
        presentation_timestamp: Time,
    ) -> Result<(), DecodingError>;

    /// Reset the video decoder and discard all frames.
    fn reset(&mut self) -> Result<(), DecodingError>;

    /// Return and clear the latest error that happened during decoding.
    fn take_error(&mut self) -> Option<TimedDecodingError>;
}

/// Decode video to a texture.
///
/// If you want to sample multiple points in a video simultaneously, use multiple decoders.
pub struct VideoDecoder {
    data: Arc<re_video::VideoData>,
    chunk_decoder: Box<dyn VideoChunkDecoder>,

    video_texture: VideoTexture,

    current_gop_idx: usize,
    current_sample_idx: usize,

    /// Last error that was encountered during decoding.
    ///
    /// Only resets after a successful decode.
    last_error: Option<TimedDecodingError>,
}

impl VideoDecoder {
    pub fn new(
        debug_name: &str,
        render_ctx: &RenderContext,
        data: Arc<re_video::VideoData>,
        hw_acceleration: DecodeHardwareAcceleration,
    ) -> Result<Self, DecodingError> {
        // We need these allows due to `cfg_if`
        #![allow(
            clippy::needless_pass_by_value,
            clippy::needless_return,
            clippy::unnecessary_wraps,
            unused
        )]

        let debug_name = format!(
            "{debug_name}, codec: {}",
            data.human_readable_codec_string()
        );

        if let Some(bit_depth) = data.config.stsd.contents.bit_depth() {
            #[allow(clippy::comparison_chain)]
            if bit_depth < 8 {
                re_log::warn_once!("{debug_name} has unusual bit_depth of {bit_depth}");
            } else if 8 < bit_depth {
                re_log::warn_once!("{debug_name}: HDR videos not supported. See https://github.com/rerun-io/rerun/issues/7594 for more.");
            }
        }

        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let decoder = web::WebVideoDecoder::new(data.clone(), hw_acceleration)?;
                return Ok(Self::from_chunk_decoder(render_ctx, data, decoder));
            } else {
                // Native AV1 video decoding:

                if !data.config.is_av1() {
                    return Err(DecodingError::UnsupportedCodec {
                        codec: data.human_readable_codec_string(),
                    });
                }

                if cfg!(debug_assertions) {
                    return Err(DecodingError::NoNativeDebug); // because debug builds of rav1d are EXTREMELY slow
                } else {
                    let av1_decoder = re_video::decode::av1::SyncDav1dDecoder::new(debug_name.clone())
                        .map_err(|err| DecodingError::StartDecoder(err.to_string()))?;

                    let decoder = native_decoder::NativeDecoder::new(debug_name, Box::new(av1_decoder))?;
                    return Ok(Self::from_chunk_decoder(render_ctx, data, decoder));
                };
            }
        }
    }

    #[allow(unused)] // Unused for certain build flags
    fn from_chunk_decoder(
        render_ctx: &RenderContext,
        data: Arc<re_video::VideoData>,
        chunk_decoder: impl VideoChunkDecoder,
    ) -> Self {
        let texture = alloc_video_frame_texture(
            &render_ctx.device,
            &render_ctx.gpu_resources.textures,
            data.config.coded_width as u32,
            data.config.coded_height as u32,
        );

        Self {
            data,
            chunk_decoder: Box::new(chunk_decoder),

            video_texture: VideoTexture {
                texture,
                time_range: Time::MAX..Time::MAX,
            },

            current_gop_idx: usize::MAX,
            current_sample_idx: usize::MAX,

            last_error: None,
        }
    }

    /// Get the video frame at the given time stamp.
    ///
    /// This will seek in the video if needed.
    /// If you want to sample multiple points in a video simultaneously, use multiple decoders.
    pub fn frame_at(
        &mut self,
        render_ctx: &RenderContext,
        presentation_timestamp_s: f64,
    ) -> Result<VideoFrameTexture, DecodingError> {
        if presentation_timestamp_s < 0.0 {
            return Err(DecodingError::NegativeTimestamp);
        }
        let presentation_timestamp = Time::from_secs(presentation_timestamp_s, self.data.timescale);
        let presentation_timestamp = presentation_timestamp.min(self.data.duration); // Don't seek past the end of the video.

        let error_on_last_frame_at = self.last_error.is_some();
        let result = self.frame_at_internal(render_ctx, presentation_timestamp);

        match result {
            Ok(()) => {
                let is_active_frame = self
                    .video_texture
                    .time_range
                    .contains(&presentation_timestamp);

                let is_pending = !is_active_frame;
                if is_pending && error_on_last_frame_at {
                    // If we switched from error to pending, clear the texture.
                    // This is important to avoid flickering, in particular when switching from
                    // benign errors like DecodingError::NegativeTimestamp.
                    // If we don't do this, we see the last valid texture which can look really weird.
                    clear_texture(render_ctx, &self.video_texture.texture);
                    self.video_texture.time_range = Time::MAX..Time::MAX;
                }

                let show_spinner = if presentation_timestamp < self.video_texture.time_range.start {
                    // We're seeking backwards and somehow forgot to reset.
                    true
                } else if presentation_timestamp < self.video_texture.time_range.end {
                    false // it is an active frame
                } else {
                    let how_outdated = presentation_timestamp - self.video_texture.time_range.end;
                    if how_outdated.into_secs(self.data.timescale)
                        < DECODING_GRACE_DELAY.as_secs_f64()
                    {
                        false // Just outdated by a little bit - show no spinner
                    } else {
                        true // Very old frame - show spinner
                    }
                };

                Ok(VideoFrameTexture {
                    texture: self.video_texture.texture.clone(),
                    time_range: self.video_texture.time_range.clone(),
                    is_pending,
                    show_spinner,
                })
            }

            Err(err) => Err(err),
        }
    }

    fn frame_at_internal(
        &mut self,
        render_ctx: &RenderContext,
        presentation_timestamp: Time,
    ) -> Result<(), DecodingError> {
        re_tracing::profile_function!();

        // Some terminology:
        //   - presentation timestamp = composition timestamp
        //     = the time at which the frame should be shown
        //   - decode timestamp
        //     = determines the decoding order of samples
        //
        // Note: `decode <= composition` for any given sample.
        //       For some codecs, the two timestamps are the same.
        // We must enqueue samples in decode order, but show them in composition order.

        // 1. Find the latest sample where `decode_timestamp <= presentation_timestamp`.
        //    Because `decode <= composition`, we never have to look further ahead in the
        //    video than this.
        let Some(decode_sample_idx) = latest_at_idx(
            &self.data.samples,
            |sample| sample.decode_timestamp,
            &presentation_timestamp,
        ) else {
            return Err(DecodingError::EmptyVideo);
        };

        // 2. Search _backwards_, starting at `decode_sample_idx`, looking for
        //    the first sample where `sample.composition_timestamp <= presentation_timestamp`.
        //    This is the sample which when decoded will be presented at the timestamp the user requested.
        let Some(requested_sample_idx) = self.data.samples[..=decode_sample_idx]
            .iter()
            .rposition(|sample| sample.composition_timestamp <= presentation_timestamp)
        else {
            return Err(DecodingError::EmptyVideo);
        };

        // 3. Do a binary search through GOPs by the decode timestamp of the found sample
        //    to find the GOP that contains the sample.
        let Some(requested_gop_idx) = latest_at_idx(
            &self.data.gops,
            |gop| gop.start,
            &self.data.samples[requested_sample_idx].decode_timestamp,
        ) else {
            return Err(DecodingError::EmptyVideo);
        };

        // 4. Enqueue GOPs as needed.

        // First, check for decoding errors that may have been set asynchronously and reset if it's a new error.
        if self.last_error.is_none() {
            if let Some(error) = self.chunk_decoder.take_error() {
                // For each new (!) error after entering the error state, we reset the decoder.
                // This way, it might later recover from the error as we progress in the video.
                //
                // By resetting the current GOP/sample indices, the frame enqueued code below
                // is forced to reset the decoder.
                self.current_gop_idx = usize::MAX;
                self.current_sample_idx = usize::MAX;
                self.last_error = Some(error);
            }
        }

        // We maintain a buffer of 2 GOPs, so we can always smoothly transition to the next GOP.
        // We can always start decoding from any GOP, because GOPs always begin with a keyframe.
        //
        // Backward seeks or seeks across many GOPs trigger a reset of the decoder,
        // because decoding all the samples between the previous sample and the requested
        // one would mean decoding and immediately discarding more frames than we need.
        if requested_gop_idx != self.current_gop_idx {
            if self.current_gop_idx.saturating_add(1) == requested_gop_idx {
                // forward seek to next GOP - queue up the one _after_ requested
                self.enqueue_gop(requested_gop_idx + 1)?;
            } else {
                // forward seek by N>1 OR backward seek across GOPs - reset
                self.reset()?;
                self.enqueue_gop(requested_gop_idx)?;
                self.enqueue_gop(requested_gop_idx + 1)?;
            }
        } else if requested_sample_idx != self.current_sample_idx {
            // special case: handle seeking backwards within a single GOP
            // this is super inefficient, but it's the only way to handle it
            // while maintaining a buffer of only 2 GOPs
            if requested_sample_idx < self.current_sample_idx {
                self.reset()?;
                self.enqueue_gop(requested_gop_idx)?;
                self.enqueue_gop(requested_gop_idx + 1)?;
            }
        }

        self.current_gop_idx = requested_gop_idx;
        self.current_sample_idx = requested_sample_idx;

        let result = self.chunk_decoder.update_video_texture(
            render_ctx,
            &mut self.video_texture,
            presentation_timestamp,
        );

        if let Err(err) = result {
            if err == DecodingError::EmptyBuffer {
                // No buffered frames

                // Might this be due to an error?
                //
                // We only care about decoding errors when we don't find the requested frame,
                // since we want to keep playing the video fine even if parts of it are broken.
                // That said, practically we reset the decoder and thus all frames upon error,
                // so it doesn't make a lot of difference.
                if let Some(timed_error) = &self.last_error {
                    if DECODING_GRACE_DELAY <= timed_error.time_of_first_error.elapsed() {
                        // Report the error only if we have been in an error state for a certain amount of time.
                        // Don't immediately report the error, since we might immediately recover from it.
                        // Otherwise, this would cause aggressive flickering!
                        return Err(timed_error.latest_error.clone());
                    }
                }

                // Don't return a zeroed texture, because we may just be behind on decoding
                // and showing an old frame is better than showing a blank frame,
                // because it causes "black flashes" to appear
                Ok(())
            } else {
                Err(err)
            }
        } else {
            self.last_error = None;
            Ok(())
        }
    }

    /// Enqueue all samples in the given GOP.
    ///
    /// Does nothing if the index is out of bounds.
    fn enqueue_gop(&mut self, gop_idx: usize) -> Result<(), DecodingError> {
        let Some(gop) = self.data.gops.get(gop_idx) else {
            return Ok(());
        };

        let samples = &self.data.samples[gop.range()];

        for (i, sample) in samples.iter().enumerate() {
            let chunk = self.data.get(sample).ok_or(DecodingError::BadData)?;
            let is_keyframe = i == 0;
            self.chunk_decoder.decode(chunk, is_keyframe)?;
        }

        Ok(())
    }

    /// Reset the video decoder and discard all frames.
    fn reset(&mut self) -> Result<(), DecodingError> {
        self.chunk_decoder.reset()?;
        self.current_gop_idx = usize::MAX;
        self.current_sample_idx = usize::MAX;
        // Do *not* reset the error state. We want to keep track of the last error.
        Ok(())
    }
}

#[allow(unused)] // For some feature flags
fn alloc_video_frame_texture(
    device: &wgpu::Device,
    pool: &GpuTexturePool,
    width: u32,
    height: u32,
) -> GpuTexture2D {
    let Some(texture) = GpuTexture2D::new(pool.alloc(
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
    )) else {
        // We set the dimension to `2D` above, so this should never happen.
        unreachable!();
    };

    texture
}

/// Clears the texture that is shown on pending to black.
fn clear_texture(render_ctx: &RenderContext, texture: &GpuTexture2D) {
    // Clear texture is a native only feature, so let's not do that.
    // before_view_builder_encoder.clear_texture(texture, subresource_range);

    // But our target is also a render target, so just create a dummy renderpass with clear.
    let mut before_view_builder_encoder =
        render_ctx.active_frame.before_view_builder_encoder.lock();
    let _ = before_view_builder_encoder
        .get()
        .begin_render_pass(&wgpu::RenderPassDescriptor {
            label: crate::DebugLabel::from("clear_video_texture").get(),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &texture.default_view,
                resolve_target: None,
                ops: wgpu::Operations::<wgpu::Color> {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
}

/// Returns the index of:
/// - The index of `needle` in `v`, if it exists
/// - The index of the first element in `v` that is lesser than `needle`, if it exists
/// - `None`, if `v` is empty OR `needle` is greater than all elements in `v`
#[allow(unused)] // For some feature flags
fn latest_at_idx<T, K: Ord>(v: &[T], key: impl Fn(&T) -> K, needle: &K) -> Option<usize> {
    if v.is_empty() {
        return None;
    }

    let idx = v.partition_point(|x| key(x) <= *needle);

    if idx == 0 {
        // If idx is 0, then all elements are greater than the needle
        if &key(&v[0]) > needle {
            return None;
        }
    }

    Some(idx.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latest_at_idx() {
        let v = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        assert_eq!(latest_at_idx(&v, |v| *v, &0), None);
        assert_eq!(latest_at_idx(&v, |v| *v, &1), Some(0));
        assert_eq!(latest_at_idx(&v, |v| *v, &2), Some(1));
        assert_eq!(latest_at_idx(&v, |v| *v, &3), Some(2));
        assert_eq!(latest_at_idx(&v, |v| *v, &4), Some(3));
        assert_eq!(latest_at_idx(&v, |v| *v, &5), Some(4));
        assert_eq!(latest_at_idx(&v, |v| *v, &6), Some(5));
        assert_eq!(latest_at_idx(&v, |v| *v, &7), Some(6));
        assert_eq!(latest_at_idx(&v, |v| *v, &8), Some(7));
        assert_eq!(latest_at_idx(&v, |v| *v, &9), Some(8));
        assert_eq!(latest_at_idx(&v, |v| *v, &10), Some(9));
        assert_eq!(latest_at_idx(&v, |v| *v, &11), Some(9));
        assert_eq!(latest_at_idx(&v, |v| *v, &1000), Some(9));
    }
}
