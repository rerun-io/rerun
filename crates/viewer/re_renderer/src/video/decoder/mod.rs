#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(feature = "video_av1")]
#[cfg(not(target_arch = "wasm32"))]
mod native_av1;

use std::{sync::Arc, time::Duration};

use web_time::Instant;

use re_video::{Chunk, Time};

use crate::{
    resource_managers::GpuTexture2D,
    wgpu_resources::{GpuTexturePool, TextureDesc},
    RenderContext,
};

use super::{DecodeHardwareAcceleration, DecodingError, VideoFrameTexture};

/// Delaying error reports (and showing last-good images meanwhile) allows us to skip over
/// transient errors without flickering.
#[allow(unused)] // Unused for certain build flags
pub const DECODING_ERROR_REPORTING_DELAY: Duration = Duration::from_millis(400);

#[allow(unused)] // Unused for certain build flags
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

/// Decode video to a texture.
///
/// If you want to sample multiple points in a video simultaneously, use multiple decoders.
trait VideoChunkDecoder: 'static + Send {
    /// Start decoding the given chunk.
    fn decode(&mut self, chunk: Chunk, is_keyframe: bool) -> Result<(), DecodingError>;

    /// Get the latest decoded frame at the given time,
    /// and drop all earlier frames to save memory.
    fn latest_at(
        &mut self,
        presentation_timestamp: Time,
    ) -> Result<VideoFrameTexture, DecodingError>;

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

    current_segment_idx: usize,
    current_sample_idx: usize,

    error: Option<TimedDecodingError>,
}

impl VideoDecoder {
    pub fn new(
        debug_name: &str,
        render_context: &RenderContext,
        data: Arc<re_video::VideoData>,
        hw_acceleration: DecodeHardwareAcceleration,
    ) -> Result<Self, DecodingError> {
        #![allow(unused, clippy::unnecessary_wraps, clippy::needless_pass_by_value)] // only for some feature flags

        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let decoder = web::WebVideoDecoder::new(render_context, data, hw_acceleration)?;
                return Ok(Self::from_decoder(data, decoder));
            } else if #[cfg(feature = "video_av1")] {
                if cfg!(debug_assertions) {
                    return Err(DecodingError::NoNativeDebug); // because debug builds of rav1d are so slow
                } else {
                    let decoder = native_av1::Av1VideoDecoder::new(debug_name, render_context, data.clone())?;
                    return Ok(Self::from_chunk_decoder(data, decoder));
                };
            } else {
                Err(DecodingError::NoNativeSupport)
            }
        }
    }

    #[allow(unused)] // Unused for certain build flags
    fn from_chunk_decoder(
        data: Arc<re_video::VideoData>,
        chunk_decoder: impl VideoChunkDecoder,
    ) -> Self {
        Self {
            data,
            chunk_decoder: Box::new(chunk_decoder),

            current_segment_idx: usize::MAX,
            current_sample_idx: usize::MAX,

            error: None,
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
        re_tracing::profile_function!();

        // Some terminology:
        //   - presentation timestamp = composition timestamp
        //     = the time at which the frame should be shown
        //   - decode timestamp
        //     = determines the decoding order of samples
        //
        // Note: `composition >= decode` for any given sample.
        //       For some codecs, the two timestamps are the same.
        // We must enqueue samples in decode order, but show them in composition order.

        if presentation_timestamp_s < 0.0 {
            return Err(DecodingError::NegativeTimestamp);
        }
        let presentation_timestamp = Time::from_secs(presentation_timestamp_s, self.data.timescale);
        let presentation_timestamp = presentation_timestamp.min(self.data.duration); // Don't seek past the end of the video.

        // 1. Find the latest sample where `decode_timestamp <= presentation_timestamp`.
        //    Because `composition >= decode`, we never have to look further ahead in the
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

        // 3. Do a binary search through segments by the decode timestamp of the found sample
        //    to find the segment that contains the sample.
        let Some(requested_segment_idx) = latest_at_idx(
            &self.data.segments,
            |segment| segment.start,
            &self.data.samples[requested_sample_idx].decode_timestamp,
        ) else {
            return Err(DecodingError::EmptyVideo);
        };

        // 4. Enqueue segments as needed.

        // First, check for decoding errors that may have been set asynchronously and reset if it's a new error.
        if let Some(error) = self.chunk_decoder.take_error() {
            // For each new (!) error after entering the error state, we reset the decoder.
            // This way, it might later recover from the error as we progress in the video.
            //
            // By resetting the current segment/sample indices, the frame enqueued code below
            // is forced to reset the decoder.
            self.current_segment_idx = usize::MAX;
            self.current_sample_idx = usize::MAX;

            self.error = Some(error);
        }

        // We maintain a buffer of 2 segments, so we can always smoothly transition to the next segment.
        // We can always start decoding from any segment, because segments always begin with a keyframe.
        //
        // Backward seeks or seeks across many segments trigger a reset of the decoder,
        // because decoding all the samples between the previous sample and the requested
        // one would mean decoding and immediately discarding more frames than we need.
        if requested_segment_idx != self.current_segment_idx {
            if self.current_segment_idx.saturating_add(1) == requested_segment_idx {
                // forward seek to next segment - queue up the one _after_ requested
                self.enqueue_segment(requested_segment_idx + 1)?;
            } else {
                // forward seek by N>1 OR backward seek across segments - reset
                self.reset()?;
                self.enqueue_segment(requested_segment_idx)?;
                self.enqueue_segment(requested_segment_idx + 1)?;
            }
        } else if requested_sample_idx != self.current_sample_idx {
            // special case: handle seeking backwards within a single segment
            // this is super inefficient, but it's the only way to handle it
            // while maintaining a buffer of only 2 segments
            if requested_sample_idx < self.current_sample_idx {
                self.reset()?;
                self.enqueue_segment(requested_segment_idx)?;
                self.enqueue_segment(requested_segment_idx + 1)?;
            }
        }

        self.current_segment_idx = requested_segment_idx;
        self.current_sample_idx = requested_sample_idx;

        self.chunk_decoder.latest_at(presentation_timestamp)
    }

    /// Enqueue all samples in the given segment.
    ///
    /// Does nothing if the index is out of bounds.
    fn enqueue_segment(&mut self, segment_idx: usize) -> Result<(), DecodingError> {
        let Some(segment) = self.data.segments.get(segment_idx) else {
            return Ok(());
        };

        let samples = &self.data.samples[segment.range()];

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
        self.error = None;
        self.current_segment_idx = usize::MAX;
        self.current_sample_idx = usize::MAX;
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
