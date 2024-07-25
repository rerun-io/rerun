use crate::resource_managers::GpuTexture2D;
use crate::wgpu_resources::{GpuTexturePool, TextureDesc};
use crate::RenderContext;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::task::Poll;

use parking_lot::Mutex;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast as _;
use web_sys::window;
use web_sys::Event;
use web_sys::HtmlVideoElement;
use wgpu::Device;
use wgpu::Queue;

struct AtomicF64(AtomicU64);

impl AtomicF64 {
    fn new(v: f64) -> Self {
        Self(AtomicU64::new(v.to_bits()))
    }

    fn set(&self, v: f64) {
        self.0.store(v.to_bits(), Ordering::SeqCst);
    }

    fn get(&self) -> f64 {
        f64::from_bits(self.0.load(Ordering::SeqCst))
    }
}

pub struct Video {
    url: String,

    device: Arc<Device>,
    queue: Arc<Queue>,

    video: HtmlVideoElement,

    /// Cached texture for a specific time in the video.
    texture: Mutex<Option<GpuTexture2D>>,
    current_time: AtomicF64,
}

impl Video {
    pub fn load(render_context: &RenderContext, url: String) -> Self {
        let window = window().expect("failed to get window");
        let document = window.document().expect("failed to get document");
        let video = document
            .create_element("video")
            .expect("failed to create video element");
        let video: HtmlVideoElement = video.dyn_into().expect("failed to create video element");

        // Without this, the bytes of the video can't be read directly
        // in case the video comes from a different domain.
        video.set_cross_origin(Some("anonymous"));
        // Without this, the video is not a valid media source for `copyExternalImageToTexture`.
        video.set_preload("auto");

        video.set_src(&url);
        video.load();

        Self {
            url,

            device: render_context.device.clone(),
            queue: render_context.queue.clone(),

            video,

            texture: Mutex::new(None),
            current_time: AtomicF64::new(f64::MAX),
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
