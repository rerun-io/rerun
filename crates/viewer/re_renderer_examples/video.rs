//! Examples for using 2D rendering.
//!
//! On the left is a 2D view, on the right a 3D view of the same scene.

// TODO(#3408): remove unwrap()
#![allow(clippy::unwrap_used)]

mod framework;

#[cfg(target_arch = "wasm32")]
fn main() {
    use std::task::Poll;

    use re_renderer::renderer::ColormappedTexture;
    use re_renderer::renderer::RectangleDrawData;
    use re_renderer::renderer::RectangleOptions;
    use re_renderer::renderer::TextureFilterMag;
    use re_renderer::renderer::TextureFilterMin;
    use re_renderer::renderer::TexturedRect;
    use re_renderer::resource_managers::VideoHandle;
    use re_renderer::view_builder::OrthographicCameraMode;
    use re_renderer::view_builder::Projection;
    use re_renderer::view_builder::TargetConfiguration;
    use re_renderer::RenderContext;
    use re_renderer::ViewBuilder;
    use web_time::Instant;
    use winit::keyboard::Key;
    use winit::keyboard::NamedKey;

    pub struct Example {
        current_time: f64,
        previous_frame_time: Instant,
        playing: bool,
        handle: VideoHandle,
    }

    impl framework::Example for Example {
        fn title() -> &'static str {
            "Video playback"
        }

        fn new(re_ctx: &RenderContext) -> Self {
            let url = "https://static.rerun.io/a510da2bddbbb13b011f3563682a0df0dcb7fbfa_visualize_and_interact_1080p.mp4".to_owned();
            let handle = re_ctx.video_manager.write().create(re_ctx, url);
            Example {
                current_time: 0.0,
                previous_frame_time: Instant::now(),
                playing: false,

                handle,
            }
        }

        fn draw(
            &mut self,
            re_ctx: &RenderContext,
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

            {
                let video_manager = re_ctx.video_manager.read();
                let video = video_manager.get(&self.handle).unwrap();

                if let Poll::Ready((_, texture)) = video.texture(&re_ctx.gpu_resources.textures) {
                    view_builder.queue_draw(
                        RectangleDrawData::new(
                            re_ctx,
                            &[TexturedRect {
                                top_left_corner_position: [0.0; 3].into(),
                                extent_u: resolution[1] as f32
                                    * video.aspect_ratio()
                                    * glam::Vec3::X,
                                extent_v: resolution[1] as f32 * glam::Vec3::Y,
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
                }
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

        fn on_key_event(&mut self, input: winit::event::KeyEvent) {
            if input.state.is_pressed() {
                return;
            }

            if matches!(input.logical_key, Key::Named(NamedKey::Space)) {
                self.playing ^= true;
                return;
            }

            const APPROX_ONE_FRAME: f64 = 1.0 / 24.0;

            let current_time = self.current_time;
            let new_time = match input.logical_key {
                Key::Named(NamedKey::ArrowRight) => current_time + APPROX_ONE_FRAME,
                Key::Named(NamedKey::ArrowLeft) => current_time - APPROX_ONE_FRAME,
                _ => current_time,
            };

            if current_time != new_time {
                re_log::debug!("seek right {current_time:?} -> {new_time:?}");
                self.current_time = new_time;
            }
        }
    }

    framework::start::<Example>();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    panic!("this demo is web-only")
}
