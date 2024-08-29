use re_video::VideoLoadError;

use crate::resource_managers::GpuTexture2D;
use crate::wgpu_resources::GpuTexturePool;
use crate::wgpu_resources::TextureDesc;
use crate::RenderContext;

use re_video::TimeMs;

pub struct Video {
    decoder: decoder::VideoDecoder,
}

impl Video {
    pub fn load(
        render_context: &RenderContext,
        media_type: Option<&str>,
        data: &[u8],
    ) -> Result<Self, VideoError> {
        let data = match media_type {
            Some("video/mp4") => re_video::load_mp4(data)?,
            Some(media_type) => {
                return Err(VideoError::Load(VideoLoadError::UnsupportedMediaType(
                    media_type.to_owned(),
                )))
            }
            None => return Err(VideoError::Load(VideoLoadError::UnknownMediaType)),
        };
        let decoder = VideoDecoder::new(render_context, data).ok_or_else(|| VideoError::Init)?;

        Ok(Self { decoder })
    }

    pub fn duration_ms(&self) -> f64 {
        self.decoder.duration_ms()
    }

    pub fn width(&self) -> u32 {
        self.decoder.width()
    }

    pub fn height(&self) -> u32 {
        self.decoder.height()
    }

    /// Returns a texture with the latest frame at the given timestamp.
    ///
    /// If the timestamp is negative, a zeroed texture is returned.
    pub fn get_frame(&mut self, timestamp_s: f64) -> GpuTexture2D {
        re_tracing::profile_function!();
        self.decoder.get_frame(TimeMs::new(timestamp_s * 1e3))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum VideoError {
    #[error("{0}")]
    Load(#[from] VideoLoadError),

    #[error("failed to initialize video decoder")]
    Init,
}

use decoder::VideoDecoder;

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
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
        },
    )) else {
        unreachable!();
    };

    texture
}

/// Returns the index of:
/// - The index of `needle` in `v`, if it exists
/// - The index of the first element in `v` that is lesser than `needle`, if it exists
/// - `None`, if `v` is empty OR `needle` is greater than all elements in `v`
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

#[cfg(target_arch = "wasm32")]
mod decoder {
    use super::latest_at_idx;
    use crate::resource_managers::GpuTexture2D;
    use crate::RenderContext;
    use js_sys::Function;
    use js_sys::Uint8Array;
    use parking_lot::Mutex;
    use re_video::TimeMs;
    use re_video::VideoData;
    use std::ops::Deref;
    use std::sync::Arc;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast as _;
    use web_sys::EncodedVideoChunk;
    use web_sys::EncodedVideoChunkInit;
    use web_sys::EncodedVideoChunkType;
    use web_sys::VideoDecoderConfig;
    use web_sys::VideoDecoderInit;

    #[derive(Clone)]
    #[repr(transparent)]
    struct VideoFrame(web_sys::VideoFrame);

    impl Drop for VideoFrame {
        fn drop(&mut self) {
            self.0.close();
        }
    }

    impl Deref for VideoFrame {
        type Target = web_sys::VideoFrame;

        #[inline]
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    pub struct VideoDecoder {
        data: re_video::VideoData,
        queue: Arc<wgpu::Queue>,
        texture: GpuTexture2D,
        zeroed_texture_float: GpuTexture2D,

        decoder: web_sys::VideoDecoder,

        frames: Arc<Mutex<Vec<(TimeMs, VideoFrame)>>>,
        last_used_frame_timestamp: TimeMs,
        current_segment_idx: usize,
        current_sample_idx: usize,
    }

    // SAFETY: There is no way to access the same JS object from different OS threads
    //         in a way that could result in a data race.

    #[allow(unsafe_code)]
    // Clippy did not recognize a safety comment on these impls no matter what I tried:
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe impl Send for VideoDecoder {}

    #[allow(unsafe_code)]
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe impl Sync for VideoDecoder {}

    #[allow(unsafe_code)]
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe impl Send for VideoFrame {}

    #[allow(unsafe_code)]
    #[allow(clippy::undocumented_unsafe_blocks)]
    unsafe impl Sync for VideoFrame {}

    impl Drop for VideoDecoder {
        fn drop(&mut self) {
            self.decoder.close();
        }
    }

    impl VideoDecoder {
        pub fn new(render_context: &RenderContext, data: VideoData) -> Option<Self> {
            let frames = Arc::new(Mutex::new(Vec::with_capacity(16)));

            let decoder = init_video_decoder({
                let frames = frames.clone();
                move |frame: web_sys::VideoFrame| {
                    frames.lock().push((
                        TimeMs::new(frame.timestamp().unwrap_or(0.0)),
                        VideoFrame(frame),
                    ));
                }
            })?;

            let queue = render_context.queue.clone();

            // NOTE: both textures are assumed to be rgba8unorm
            let texture = super::alloc_video_frame_texture(
                &render_context.device,
                &render_context.gpu_resources.textures,
                data.config.coded_width as u32,
                data.config.coded_height as u32,
            );
            let zeroed_texture_float = GpuTexture2D::new(
                render_context
                    .texture_manager_2d
                    .zeroed_texture_float()
                    .clone(),
            )
            .expect("expected texture to be 2D");

            let mut this = Self {
                data,
                queue,
                texture,
                zeroed_texture_float,

                decoder,

                frames,
                last_used_frame_timestamp: TimeMs::new(f64::MAX),
                current_segment_idx: usize::MAX,
                current_sample_idx: usize::MAX,
            };

            // immediately enqueue some frames, assuming playback at start
            this.reset();
            let _ = this.get_frame(TimeMs::new(0.0));

            Some(this)
        }

        pub fn duration_ms(&self) -> f64 {
            self.data.duration.as_f64()
        }

        pub fn width(&self) -> u32 {
            self.data.config.coded_width as u32
        }

        pub fn height(&self) -> u32 {
            self.data.config.coded_height as u32
        }

        pub fn get_frame(&mut self, timestamp: TimeMs) -> GpuTexture2D {
            if timestamp.as_f64() < 0.0 {
                return self.zeroed_texture_float.clone();
            }

            let Some(segment_idx) =
                latest_at_idx(&self.data.segments, |segment| segment.timestamp, &timestamp)
            else {
                return self.texture.clone();
            };

            let Some(sample_idx) = latest_at_idx(
                &self.data.segments[segment_idx].samples,
                |sample| sample.timestamp,
                &timestamp,
            ) else {
                // segments are never empty
                unreachable!();
            };

            if segment_idx != self.current_segment_idx {
                let segment_distance = segment_idx as isize - self.current_segment_idx as isize;
                if segment_distance == 1 {
                    // forward seek to next segment
                    self.enqueue_all(segment_idx + 1);
                } else {
                    // forward seek by N>1 OR backward seek across segments
                    self.reset();
                    self.enqueue_all(segment_idx);
                    self.enqueue_all(segment_idx + 1);
                }
            } else if sample_idx != self.current_sample_idx {
                let sample_distance = sample_idx as isize - self.current_sample_idx as isize;
                if sample_distance < 0 {
                    self.reset();
                    self.enqueue_all(segment_idx);
                    self.enqueue_all(segment_idx + 1);
                }
            }

            self.current_segment_idx = segment_idx;
            self.current_sample_idx = sample_idx;

            let mut frames = self.frames.lock();

            let Some(frame_idx) = latest_at_idx(&frames, |(t, _)| *t, &timestamp) else {
                // no buffered frames - texture will be blank
                // TODO(jan): do something less bad
                return self.texture.clone();
            };
            let frame = frames[frame_idx].1 .0.clone();

            // drain up-to (but not including) the frame idx, clearing out any frames
            // before it. this lets the video decoder output more frames.
            drop(frames.drain(0..frame_idx));

            let Some((frame_timestamp_ms, frame_duration_ms)) = frame
                .timestamp()
                .map(TimeMs::new)
                .zip(frame.duration().map(TimeMs::new))
            else {
                // TODO(jan): figure out when this can happen and handle it
                return self.texture.clone();
            };

            if TimeMs::new(timestamp.as_f64() - frame_timestamp_ms.as_f64()) > frame_duration_ms {
                // not relevant to the user, it's an old frame.
                return self.texture.clone();
            }

            if self.last_used_frame_timestamp != frame_timestamp_ms {
                copy_video_frame_to_texture(&self.queue, frame, &self.texture.texture);
                self.last_used_frame_timestamp = frame_timestamp_ms;
            }

            self.texture.clone()
        }

        fn enqueue_all(&self, segment_idx: usize) {
            let Some(segment) = self.data.segments.get(segment_idx) else {
                return;
            };

            self.enqueue(&segment.samples[0], true);
            for sample in &segment.samples[1..] {
                self.enqueue(sample, false);
            }
        }

        fn enqueue(&self, sample: &re_video::Sample, is_key: bool) {
            let data = Uint8Array::from(
                &self.data.data[sample.byte_offset as usize
                    ..sample.byte_offset as usize + sample.byte_length as usize],
            );
            let type_ = if is_key {
                EncodedVideoChunkType::Key
            } else {
                EncodedVideoChunkType::Delta
            };
            let mut chunk = EncodedVideoChunkInit::new(&data, sample.timestamp.as_f64(), type_);
            chunk.duration(sample.duration.as_f64());
            let Some(chunk) = EncodedVideoChunk::new(&chunk)
                .inspect_err(|err| {
                    re_log::error!("failed to create video chunk: {}", js_error_to_string(err));
                })
                .ok()
            else {
                return;
            };

            self.decoder.decode(&chunk);
        }

        fn reset(&mut self) {
            self.decoder.reset();
            self.decoder
                .configure(&js_video_decoder_config(&self.data.config));

            let mut frames = self.frames.lock();
            drop(frames.drain(..));
        }
    }

    fn copy_video_frame_to_texture(
        queue: &wgpu::Queue,
        frame: web_sys::VideoFrame,
        texture: &wgpu::Texture,
    ) {
        let size = wgpu::Extent3d {
            width: frame.display_width(),
            height: frame.display_height(),
            depth_or_array_layers: 1,
        };
        let source = {
            // TODO(jan): Add `VideoFrame` support to `wgpu`

            // SAFETY: Depends on the fact that `wgpu` passes the object through as-is,
            // and doesn't actually inspect it in any way. The browser then does its own
            // typecheck that doesn't care what kind of image source wgpu gave it.
            #[allow(unsafe_code)]
            let frame = unsafe {
                std::mem::transmute::<web_sys::VideoFrame, web_sys::HtmlVideoElement>(frame)
            };
            wgpu_types::ImageCopyExternalImage {
                source: wgpu_types::ExternalImageSource::HTMLVideoElement(frame),
                origin: wgpu_types::Origin2d { x: 0, y: 0 },
                flip_y: false,
            }
        };
        let dest = wgpu::ImageCopyTextureTagged {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
            aspect: wgpu::TextureAspect::All,
            color_space: wgpu::PredefinedColorSpace::Srgb,
            premultiplied_alpha: false,
        };
        queue.copy_external_image_to_texture(&source, dest, size);
    }

    fn init_video_decoder(
        on_output: impl Fn(web_sys::VideoFrame) + 'static,
    ) -> Option<web_sys::VideoDecoder> {
        let on_output = Closure::wrap(Box::new(on_output) as Box<dyn Fn(web_sys::VideoFrame)>);
        let on_error = Closure::wrap(Box::new(|err: js_sys::Error| {
            let err = std::string::ToString::to_string(&err.to_string());

            re_log::error!("failed to decode video: {err}");
        }) as Box<dyn Fn(js_sys::Error)>);

        let Ok(on_output) = on_output.into_js_value().dyn_into::<Function>() else {
            unreachable!()
        };
        let Ok(on_error) = on_error.into_js_value().dyn_into::<Function>() else {
            unreachable!()
        };
        let decoder = web_sys::VideoDecoder::new(&VideoDecoderInit::new(&on_error, &on_output))
            .inspect_err(|err| {
                re_log::error!("failed to create VideoDecoder: {}", js_error_to_string(err));
            })
            .ok()?;

        Some(decoder)
    }

    fn js_video_decoder_config(config: &re_video::Config) -> VideoDecoderConfig {
        let mut js = VideoDecoderConfig::new(&config.codec);
        js.coded_width(config.coded_width as u32);
        js.coded_height(config.coded_height as u32);
        let description = Uint8Array::new_with_length(config.description.len() as u32);
        description.copy_from(&config.description[..]);
        js.description(&description);
        js.optimize_for_latency(true);
        js
    }

    fn js_error_to_string(v: &wasm_bindgen::JsValue) -> String {
        if let Some(v) = v.as_string() {
            return v;
        }

        if let Some(v) = v.dyn_ref::<js_sys::Error>() {
            return std::string::ToString::to_string(&v.to_string());
        }

        format!("{v:#?}")
    }
}

// TODO(jan): decode on native
#[cfg(not(target_arch = "wasm32"))]
mod decoder {
    #![allow(dead_code, unused_variables, clippy::unnecessary_wraps)]

    use crate::resource_managers::GpuTexture2D;
    use crate::RenderContext;

    // TODO(jan): remove once we have native video decoding
    #[allow(unused_imports)]
    use super::latest_at_idx;

    use re_video::TimeMs;

    use super::alloc_video_frame_texture;

    pub struct VideoDecoder {
        data: re_video::VideoData,
        texture: GpuTexture2D,
    }

    impl VideoDecoder {
        pub fn new(render_context: &RenderContext, data: re_video::VideoData) -> Option<Self> {
            let device = render_context.device.clone();
            let texture = alloc_video_frame_texture(
                &device,
                &render_context.gpu_resources.textures,
                data.config.coded_width as u32,
                data.config.coded_height as u32,
            );
            Some(Self { data, texture })
        }

        pub fn duration_ms(&self) -> f64 {
            self.data.duration.as_f64()
        }

        pub fn width(&self) -> u32 {
            self.data.config.coded_width as u32
        }

        pub fn height(&self) -> u32 {
            self.data.config.coded_height as u32
        }

        pub fn get_frame(&mut self, timestamp: TimeMs) -> GpuTexture2D {
            self.texture.clone()
        }
    }
}
