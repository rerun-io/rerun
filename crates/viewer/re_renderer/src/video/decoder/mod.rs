#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
mod no_native_decoder;

#[cfg(feature = "video_av1")]
#[cfg(not(target_arch = "wasm32"))]
mod native_av1;

use crate::{
    resource_managers::GpuTexture2D,
    wgpu_resources::{GpuTexturePool, TextureDesc},
    RenderContext,
};

use std::{sync::Arc, time::Duration};

use super::{DecodeHardwareAcceleration, DecodingError, FrameDecodingResult};

/// Delaying error reports (and showing last-good images meanwhile) allows us to skip over
/// transient errors without flickering.
#[allow(unused)]
pub const DECODING_ERROR_REPORTING_DELAY: Duration = Duration::from_millis(400);

/// Decode video to a texture.
///
/// If you want to sample multiple points in a video simultaneously, use multiple decoders.
pub trait VideoDecoder: 'static + Send {
    /// Get the video frame at the given time stamp.
    ///
    /// This will seek in the video if needed.
    /// If you want to sample multiple points in a video simultaneously, use multiple decoders.
    fn frame_at(
        &mut self,
        render_ctx: &RenderContext,
        presentation_timestamp_s: f64,
    ) -> FrameDecodingResult;
}

pub fn new_video_decoder(
    render_context: &RenderContext,
    data: Arc<re_video::VideoData>,
    hw_acceleration: DecodeHardwareAcceleration,
) -> Result<Box<dyn VideoDecoder>, DecodingError> {
    #![allow(unused, clippy::unnecessary_wraps, clippy::needless_pass_by_value)] // only for some feature flags

    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let decoder = web::WebVideoDecoder::new(render_context, data, hw_acceleration)?;
        } else if #[cfg(feature = "video_av1")] {
            let decoder = native_av1::Av1VideoDecoder::new(render_context, data)?;
        } else {
            let decoder = no_native_decoder::NoNativeVideoDecoder::default();
        }
    };

    Ok(Box::new(decoder))
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
