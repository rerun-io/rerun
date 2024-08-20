use re_video::VideoLoadError;

use crate::RenderContext;

pub struct Video {
    decoder: decoder::VideoDecoder,
}

impl Video {
    pub fn load(
        render_context: &RenderContext,
        media_type: &str,
        data: &[u8],
    ) -> Result<Self, VideoError> {
        let data = match media_type {
            "video/mp4" => re_video::load_mp4(data)?,
            _ => return Err(VideoError::Load(VideoLoadError::UnknownMediaType)),
        };
        let decoder = VideoDecoder::new(render_context, data).ok_or_else(|| VideoError::Init)?;

        Ok(Self { decoder })
    }
}

#[derive(Debug)]
pub enum VideoError {
    Load(VideoLoadError),
    Init,
}

impl std::error::Error for VideoError {}

impl std::fmt::Display for VideoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Load(err) => std::fmt::Display::fmt(err, f),
            Self::Init => write!(f, "failed to initialize video decoder"),
        }
    }
}

impl From<VideoLoadError> for VideoError {
    fn from(value: VideoLoadError) -> Self {
        Self::Load(value)
    }
}

use decoder::VideoDecoder;

#[cfg(target_arch = "wasm32")]
mod decoder {
    use crate::resource_managers::GpuTexture2D;
    use crate::wgpu_resources::GpuTexturePool;
    use crate::wgpu_resources::TextureDesc;
    use crate::RenderContext;
    use js_sys::Function;
    use js_sys::Uint8Array;
    use parking_lot::Mutex;
    use re_video::VideoData;
    use std::sync::Arc;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast as _;
    use web_sys::EncodedVideoChunk;
    use web_sys::EncodedVideoChunkInit;
    use web_sys::EncodedVideoChunkType;
    use web_sys::VideoDecoderConfig;
    use web_sys::VideoDecoderInit;
    use web_sys::VideoFrame;

    type Frames = Vec<(u64, VideoFrame)>;

    pub struct VideoDecoder {
        data: re_video::VideoData,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        texture: GpuTexture2D,

        decoder: web_sys::VideoDecoder,

        frames: Arc<Mutex<Vec<(u64, VideoFrame)>>>,
        last_used_frame_timestamp: u64,
        current_segment_idx: usize,
        current_sample_idx: usize,
    }

    impl VideoDecoder {
        pub fn new(render_context: &RenderContext, data: VideoData) -> Option<Self> {
            let frames = Arc::new(Mutex::new(Vec::with_capacity(16)));

            let device = render_context.device.clone();
            let queue = render_context.queue.clone();

            let decoder = init_video_decoder({
                let frames = frames.clone();
                move |frame: VideoFrame| {
                    frames
                        .lock()
                        .push((frame.timestamp().unwrap_or(0.0) as u64, frame));
                }
            })?;

            let texture = alloc_video_frame_texture(
                &device,
                &render_context.gpu_resources.textures,
                data.config.coded_width as u32,
                data.config.coded_height as u32,
            );

            let mut this = Self {
                data,
                device,
                queue,
                texture,

                decoder,

                frames,
                last_used_frame_timestamp: u64::MAX,
                current_segment_idx: usize::MAX,
                current_sample_idx: usize::MAX,
            };

            // immediately enqueue some frames, assuming playback at start
            this.reset();
            let _ = this.get_frame(0.0);

            Some(this)
        }

        pub fn get_frame(&mut self, timestamp_s: f64) -> GpuTexture2D {
            let timestamp = (timestamp_s * self.data.timescale as f64).floor() as u64;
            self.try_buffer_frames(timestamp);

            let frames = self.frames.lock();
            let Some(frame_idx) = latest_at_idx(&frames, |(t, _)| *t, timestamp) else {
                // no buffered frames - texture will be blank
                // TODO(jan): do something less bad
                return self.texture.clone();
            };
            drop(frames);

            self.clear_old_frames(frame_idx);
            self.update_texture(timestamp, frame_idx);

            self.texture.clone()
        }

        fn clear_old_frames(&mut self, frame_idx: usize) {
            let mut frames = self.frames.lock();
            // drain up-to (but not including) the frame idx, clearing out any frames
            // before it. this lets the video decoder output more frames.
            for (_, frame) in frames.drain(0..frame_idx) {
                frame.close();
            }
        }

        fn update_texture(&mut self, timestamp: u64, frame_idx: usize) {
            let frames = self.frames.lock();
            let frame = frames[frame_idx].1.clone();
            drop(frames);

            let Some((frame_timestamp, frame_duration)) = frame
                .timestamp()
                .map(|v| v as u64)
                .zip(frame.duration().map(|v| v as u64))
            else {
                // TODO(jan): figure out when this can happen and handle it
                return;
            };

            if timestamp - frame_timestamp > frame_duration {
                // not relevant to the user, it's an old frame.
                return;
            }

            if self.last_used_frame_timestamp != frame_timestamp {
                copy_video_frame_to_texture(&self.queue, frame, &self.texture.texture);
                self.last_used_frame_timestamp = frame_timestamp;
            }
        }

        fn try_buffer_frames(&mut self, timestamp: u64) {
            let Some(segment_idx) =
                latest_at_idx(&self.data.segments, |segment| segment.timestamp, timestamp)
            else {
                return;
            };

            let Some(sample_idx) = latest_at_idx(
                &self.data.segments[segment_idx].samples,
                |sample| sample.timestamp,
                timestamp,
            ) else {
                // segments are never empty
                unreachable!();
            };

            if segment_idx != self.current_segment_idx {
                let segment_distance = segment_idx as isize - self.current_segment_idx as isize;
                if segment_distance == 1 {
                    // forward seek to next segment
                    self.enqueue_all(segment_idx);
                } else {
                    // forward seek by N>1 OR backward seek across segments
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
            let Some(chunk) = EncodedVideoChunk::new(&EncodedVideoChunkInit::new(
                &data,
                sample.timestamp as f64,
                type_,
            ))
            .inspect_err(|err| {
                re_log::error!("failed to create video chunk: {}", js_error_to_string(&err))
            })
            .ok() else {
                return;
            };

            self.decoder.decode(&chunk);
        }

        fn reset(&mut self) {
            self.decoder.reset();
            self.decoder
                .configure(&js_video_decoder_config(&self.data.config));

            let mut frames = self.frames.lock();
            for (_, frame) in frames.drain(..) {
                frame.close()
            }
        }
    }

    fn latest_at_idx<T, K: Ord>(v: &[T], key: impl Fn(&T) -> K, needle: K) -> Option<usize> {
        if v.is_empty() {
            return None;
        }

        match v.binary_search_by_key(&needle, key) {
            Ok(idx) => Some(idx),
            Err(idx) => {
                if idx == 0 {
                    return None;
                }

                Some(idx - 1)
            }
        }
    }

    fn latest_at<T, K: Ord>(v: &[T], key: impl Fn(&T) -> K, needle: K) -> Option<&T> {
        latest_at_idx(v, key, needle).map(|idx| &v[idx])
    }

    fn alloc_video_frame_texture(
        device: &wgpu::Device,
        pool: &GpuTexturePool,
        width: u32,
        height: u32,
    ) -> GpuTexture2D {
        let Some(texture) = GpuTexture2D::new(pool.alloc(
            &device,
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

    fn copy_video_frame_to_texture(
        queue: &wgpu::Queue,
        frame: VideoFrame,
        texture: &wgpu::Texture,
    ) {
        let size = wgpu::Extent3d {
            width: frame.display_width(),
            height: frame.display_height(),
            depth_or_array_layers: 1,
        };
        let source = wgpu_types::ImageCopyExternalImage {
            // SAFETY: Depends on the fact that `wgpu` passes the object through as-is,
            // and doesn't actually inspect it in any way. The browser then does its own
            // typecheck that doesn't care what kind of image source wgpu gave it.
            // TODO(jan): Add `VideoFrame` support to `wgpu`
            #[allow(unsafe_code)]
            source: wgpu_types::ExternalImageSource::HTMLVideoElement(unsafe {
                std::mem::transmute(frame)
            }),
            origin: wgpu_types::Origin2d { x: 0, y: 0 },
            flip_y: false,
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

    struct CurrentFrameId {
        segment: u64,
        sample: u64,
    }

    fn init_video_decoder(
        on_output: impl Fn(VideoFrame) + 'static,
    ) -> Option<web_sys::VideoDecoder> {
        let on_output = Closure::wrap(Box::new(on_output) as Box<dyn Fn(VideoFrame)>);
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
        let decoder = web_sys::VideoDecoder::new(&VideoDecoderInit::new(&on_output, &on_error))
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
        js.description(&Uint8Array::from(&config.description[..]));
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

    use crate::RenderContext;

    pub struct VideoDecoder {
        data: re_video::VideoData,
    }

    impl VideoDecoder {
        pub fn new(render_context: &RenderContext, data: re_video::VideoData) -> Option<Self> {
            Some(Self { data })
        }
    }
}
