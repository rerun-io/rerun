use std::sync::Arc;
use std::sync::Mutex;
use std::task::Poll;

use re_renderer::renderer::ColormappedTexture;
use re_renderer::renderer::RectangleDrawData;
use re_renderer::renderer::RectangleOptions;
use re_renderer::renderer::TextureFilterMag;
use re_renderer::renderer::TextureFilterMin;
use re_renderer::renderer::TexturedRect;
use re_renderer::resource_managers::GpuTexture2D;
use re_renderer::view_builder::OrthographicCameraMode;
use re_renderer::view_builder::Projection;
use re_renderer::view_builder::TargetConfiguration;
use re_renderer::wgpu_resources::{GpuTexturePool, TextureDesc};
use re_renderer::ViewBuilder;
use wasm_bindgen::JsCast as _;
use web_sys::window;
use web_sys::HtmlVideoElement;
use wgpu::Device;
use wgpu::Queue;

use crate::framework;

struct Video {
    device: Arc<Device>,
    queue: Arc<Queue>,

    video: HtmlVideoElement,

    /// Cached texture for a specific time in the video.
    cache: Mutex<Option<(f64, GpuTexture2D)>>,
}

impl Video {
    fn load(render_context: &re_renderer::RenderContext, url: &str) -> Self {
        let video: HtmlVideoElement = window()
            .unwrap()
            .document()
            .unwrap()
            .create_element("video")
            .unwrap()
            .dyn_into()
            .unwrap();

        video.set_src(url);

        Self {
            device: render_context.device.clone(),
            queue: render_context.queue.clone(),

            video,

            cache: Mutex::new(None),
        }
    }

    fn seek(&self, v: f64) {
        self.video.set_current_time(v);
    }

    fn get_texture(&self, texture_pool: &GpuTexturePool) -> Poll<GpuTexture2D> {
        let current_time = self.video.current_time();

        // We already have the video loaded into the texture at this timestamp
        if let Some((time, texture)) = &*self.cache.lock().unwrap() {
            if current_time == *time {
                return Poll::Ready(texture.clone());
            }
        }

        /// https://developer.mozilla.org/en-US/docs/Web/API/HTMLMediaElement/readyState#htmlmediaelement.have_current_data
        /// Data is available for the current playback position, but not enough to actually play more than one frame.
        const HAVE_CURRENT_DATA: u16 = 2;

        re_log::debug!("{}", self.video.ready_state());
        if self.video.ready_state() < HAVE_CURRENT_DATA {
            return Poll::Pending;
        }

        let width = self.video.video_width();
        let height = self.video.video_height();
        let video_extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        re_log::debug!("{video_extent:#?}");

        let texture_handle = texture_pool.alloc(
            &self.device,
            &TextureDesc {
                label: "video".into(),
                size: video_extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            },
        );
        let external_source = wgpu_types::ImageCopyExternalImage {
            source: wgpu_types::ExternalImageSource::HTMLVideoElement(self.video.clone()),
            origin: wgpu::Origin2d::ZERO,
            flip_y: false,
        };
        let texture_destination = wgpu::ImageCopyTextureTagged {
            texture: &texture_handle.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
            color_space: wgpu::PredefinedColorSpace::Srgb,
            premultiplied_alpha: false,
        };

        self.queue.copy_external_image_to_texture(
            &external_source,
            texture_destination,
            video_extent,
        );

        let texture_handle = GpuTexture2D::new(texture_handle).unwrap();

        self.cache
            .lock()
            .unwrap()
            .replace((current_time, texture_handle.clone()));

        Poll::Ready(texture_handle)
    }
}

pub struct RenderVideo {
    video: Video,
}

impl framework::Example for RenderVideo {
    fn title() -> &'static str {
        "2D Rendering"
    }

    fn new(re_ctx: &re_renderer::RenderContext) -> Self {
        let video = Video::load(re_ctx, "https://static.rerun.io/a510da2bddbbb13b011f3563682a0df0dcb7fbfa_visualize_and_interact_1080p.mp4");

        RenderVideo { video }
    }

    fn draw(
        &mut self,
        re_ctx: &re_renderer::RenderContext,
        resolution: [u32; 2],
        time: &framework::Time,
        pixels_per_point: f32,
    ) -> Vec<framework::ViewDrawResult> {
        let view = framework::split_resolution(resolution, 1, 1)
            .next()
            .unwrap();

        let mut view_builder = ViewBuilder::new(
            re_ctx,
            TargetConfiguration {
                name: "Video".into(),
                resolution_in_pixel: view.resolution_in_pixel,
                view_from_world: re_math::IsoTransform::IDENTITY,
                projection_from_view: Projection::Orthographic {
                    camera_mode: OrthographicCameraMode::TopLeftCornerAndExtendZ,
                    vertical_world_size: view.resolution_in_pixel[1] as f32,
                    far_plane_distance: 1000.0,
                },
                pixels_per_point,
                ..Default::default()
            },
        );

        if let Poll::Ready(texture) = self.video.get_texture(&re_ctx.gpu_resources.textures) {
            view_builder.queue_draw(
                RectangleDrawData::new(
                    re_ctx,
                    &[TexturedRect {
                        top_left_corner_position: [0.0; 3].into(),
                        extent_u: texture.width() as f32 * glam::Vec3::X,
                        extent_v: texture.height() as f32 * glam::Vec3::Y,
                        colormapped_texture: ColormappedTexture::from_unorm_rgba(texture),
                        options: RectangleOptions {
                            texture_filter_magnification: TextureFilterMag::Nearest,
                            texture_filter_minification: TextureFilterMin::Linear,
                            ..Default::default()
                        },
                    }],
                )
                .unwrap(),
            );
        } else {
            re_log::info!("pending");
        }

        let command_buffer = view_builder
            .draw(re_ctx, re_renderer::Rgba::TRANSPARENT)
            .unwrap();

        vec![framework::ViewDrawResult {
            view_builder,
            command_buffer,
            target_location: view.target_location,
        }]
    }

    fn on_key_event(&mut self, _input: winit::event::KeyEvent) {}
}
