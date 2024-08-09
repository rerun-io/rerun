use crate::resource_managers::GpuTexture2D;
use crate::wgpu_resources::{GpuTexturePool, TextureDesc};
use crate::RenderContext;
use std::io::BufReader;
use std::io::Cursor;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Poll;

use js_sys::ArrayBuffer;
use js_sys::Function;
use js_sys::Object;
use js_sys::Promise;
use js_sys::Reflect;
use js_sys::Uint8Array;
use parking_lot::Mutex;
use parking_lot::RwLock;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use web_sys::Document;
use web_sys::Event;
use web_sys::HtmlVideoElement;
use web_sys::VideoDecoder;
use web_sys::VideoDecoderInit;
use web_sys::VideoFrame;
use web_sys::Window;
use wgpu::Device;
use wgpu::Queue;

fn js_get(receiver: &JsValue, key: &str) -> Result<JsValue, JsValue> {
    Reflect::get(receiver, &JsValue::from_str(key))
}

fn js_set(obj: &JsValue, key: &str, value: impl Into<JsValue>) {
    Reflect::set(obj, &JsValue::from_str(key), &value.into()).unwrap();
}

fn window() -> Window {
    web_sys::window().expect("failed to get window")
}

struct DecodedVideoConfig {
    codec: String,
    width: u16,
    height: u16,
    duration: f64,
    format: String,
}

struct DecodedVideo {
    config: DecodedVideoConfig,
    frames: Vec<VideoFrame>,
}

fn decode_video(video_url: &str, on_done: impl FnOnce(DecodedVideo) + 'static) {
    // Awful, terrible hack to embed mp4box + a wrapper library for decoding video frames.
    // We embed JS modules as strings, then construct `Function` objects out of them,
    // run those functions to initialize the modules, and then run the `__rerun_decode_video`
    // function which is appended to the global scope.

    fn init_module_from_str(module: &str) {
        // This is equivalent to calling `eval`, but with stricter scoping:
        let f = Function::new_no_args(module);
        f.call0(&window()).expect("failed to initialize module");
    }

    fn init_modules() {
        // The order in which the modules are initialized matters, `mp4box` must come first.
        const MP4BOX_MIN_JS: &str = include_str!("./video/mp4box.all.min.js");
        const DECODE_VIDEO_JS: &str = include_str!("./video/decode_video.js");

        init_module_from_str(MP4BOX_MIN_JS);
        init_module_from_str(DECODE_VIDEO_JS);
    }

    let mut f = js_get(&window(), "__rerun_decode_video").unwrap();
    if f.is_null() || f.is_undefined() {
        // not initialized yet
        init_modules();

        f = js_get(&window(), "__rerun_decode_video").unwrap();
        if f.is_null() || f.is_undefined() {
            panic!("failed to initialize __rerun_decode_video");
        }
    }

    let f: Function = f.dyn_into().expect("__rerun_decode_video is not a Function");
    f.call1(&window(), &JsValue::from_str(video_url)).expect("__rerun_decode_video failed").dyn_into::<Promise>().then(&Closure::once(|result: JsValue| -> Result<JsValue, JsValue> {
        let config = js_get(&result, "config")?;
        let frames = js_get(&result, "frames")?;

        todo!();
    }));
}

pub struct Video {
    url: String,

    device: Arc<Device>,
    queue: Arc<Queue>,

    /// Cached video frames, sorted by timestamp.
    decoded: Arc<RwLock<Option<DecodedVideo>>>,
}

impl Video {
    pub fn load(render_context: &RenderContext, url: String) -> Self {
        let video = Arc::new(RwLock::new(None));

        decode_video(&url, {
            let video = video.clone();
            move |v| {
                *video.write() = v;
            }
        })

        Self {
            url,

            device: render_context.device.clone(),
            queue: render_context.queue.clone(),

            frames,
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// Return the duration of this video in seconds.
    ///
    /// Note that this will return `0` until [`Video::is_metadata_ready`] returns true.
    pub fn duration(&self) -> f64 {
        let duration = self.video.duration();
        if !duration.is_nan() {
            duration
        } else {
            0.0
        }
    }

    /// Return the resolution of the video in pixels.
    ///
    /// Note that this will return `[0, 0]` until [`Video::is_metadata_ready`] returns true.
    pub fn resolution(&self) -> [u32; 2] {
        [self.video.video_width(), self.video.video_height()]
    }

    pub fn aspect_ratio(&self) -> f32 {
        let [width, height] = self.resolution();

        width as f32 / height as f32
    }

    /// The video metadata is ready. The metadata is information such as the resolution (width, height) and duration.
    pub fn is_metadata_ready(&self) -> bool {
        /// https://developer.mozilla.org/en-US/docs/Web/API/HTMLMediaElement/readyState#htmlmediaelement.have_current_data
        /// Enough of the media resource has been retrieved that the metadata attributes are initialized.
        /// Seeking will no longer raise an exception.
        const HAVE_METADATA: u16 = 1;

        self.video.ready_state() >= HAVE_METADATA
    }

    /// The current frame is ready for immediate display.
    pub fn is_ready_for_display(&self) -> bool {
        /// https://developer.mozilla.org/en-US/docs/Web/API/HTMLMediaElement/readyState#htmlmediaelement.have_current_data
        /// Data is available for the current playback position, but not enough to actually play more than one frame.
        const HAVE_CURRENT_DATA: u16 = 2;

        self.video.ready_state() >= HAVE_CURRENT_DATA
    }

    /// Seek to a time in the video.
    ///
    /// Note that after calling this, the video data is not immediately available.
    pub fn set_current_time(&self, v: f64) {
        self.video.set_current_time(v);
    }

    /// Current time in the video.
    pub fn current_time(&self) -> f64 {
        self.video.current_time()
    }

    /// How fast the video should play back, as a multiplier.
    pub fn playback_rate(&self) -> f64 {
        self.video.playback_rate()
    }

    /// Determines how fast the video should play back, as a multiplier.
    ///
    /// E.g. `0.5` would play back a 30 fps video at 15 fps.
    pub fn set_playback_rate(&self, v: f64) {
        self.video.set_playback_rate(v)
    }

    /// Get a texture with the current video data in it.
    ///
    /// Which frame in a video stream corresponds to a particular timestamp is defined by the video format.
    ///
    /// ⚠ This method should be called in the same _microtask_ as any draw calls which use the returned texture!
    pub fn texture(&self, texture_pool: &GpuTexturePool) -> Poll<(VideoFrameKind, GpuTexture2D)> {
        if self.is_ready_for_display() {
            re_log::debug!("ready");

            let texture = self.get_or_create_texture(texture_pool);
            self.copy_current_frame_to_texture(&texture);
            Poll::Ready((VideoFrameKind::Fresh, texture))
        } else {
            re_log::debug!("not ready");

            if let Some(texture) = &*self.texture.lock() {
                return Poll::Ready((VideoFrameKind::Delayed, texture.clone()));
            }

            Poll::Pending
        }
    }

    fn video_extent(&self) -> wgpu::Extent3d {
        let [width, height] = self.resolution();

        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        }
    }

    fn get_or_create_texture(&self, texture_pool: &GpuTexturePool) -> GpuTexture2D {
        self.texture
            .lock()
            .get_or_insert_with(|| {
                re_log::debug!("create texture");
                GpuTexture2D::new(texture_pool.alloc(
                    &self.device,
                    &TextureDesc {
                        label: "video".into(),
                        size: self.video_extent(),
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        usage: wgpu::TextureUsages::COPY_DST
                            | wgpu::TextureUsages::TEXTURE_BINDING
                            | wgpu::TextureUsages::RENDER_ATTACHMENT,
                    },
                ))
                .expect("texture is not 2d")
            })
            .clone()
    }

    fn copy_current_frame_to_texture(&self, texture: &GpuTexture2D) {
        self.queue.copy_external_image_to_texture(
            &wgpu_types::ImageCopyExternalImage {
                source: wgpu_types::ExternalImageSource::HTMLVideoElement(self.video.clone()),
                origin: wgpu::Origin2d::ZERO,
                flip_y: false,
            },
            wgpu::ImageCopyTextureTagged {
                texture: &texture.inner.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
                color_space: wgpu::PredefinedColorSpace::Srgb,
                premultiplied_alpha: false,
            },
            self.video_extent(),
        );
    }
}

pub enum VideoFrameKind {
    /// The texture corresponds to the given timestamp.
    Fresh,

    /// The texture _may_ correspond to the given timestamp,
    /// but the data may also not be available yet, in which
    /// case it corresponds to the last available timestamp.
    Delayed,
}
