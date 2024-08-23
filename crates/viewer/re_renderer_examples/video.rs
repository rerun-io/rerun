//! Examples for using 2D rendering.
//!
//! On the left is a 2D view, on the right a 3D view of the same scene.

// TODO(#3408): remove unwrap()
#![allow(clippy::unwrap_used)]

mod framework;

#[cfg(target_arch = "wasm32")]
fn main() {
    use re_renderer::renderer::ColormappedTexture;
    use re_renderer::renderer::RectangleDrawData;
    use re_renderer::renderer::RectangleOptions;
    use re_renderer::renderer::TextureFilterMag;
    use re_renderer::renderer::TextureFilterMin;
    use re_renderer::renderer::TexturedRect;
    use re_renderer::renderer::Video;
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
        frame_rate_hz: f64,
        playing: bool,
        video: Video,
    }

    impl framework::Example for Example {
        fn title() -> &'static str {
            "Video playback"
        }

        fn new(re_ctx: &RenderContext) -> Self {
            let data = include_bytes!("./assets/bbb_video_av1_frag.mp4");
            let video = Video::load(re_ctx, Some("video/mp4"), data).unwrap();
            Example {
                current_time: 30.0,
                previous_frame_time: Instant::now(),
                frame_rate_hz: 30.0,
                playing: false,

                video,
            }
        }

        fn draw(
            &mut self,
            re_ctx: &RenderContext,
            resolution: [u32; 2],
            time: &framework::Time,
            pixels_per_point: f32,
        ) -> Vec<framework::ViewDrawResult> {
            let one_frame = 1.0 / self.frame_rate_hz;
            if self.playing && self.previous_frame_time.elapsed().as_secs_f64() > one_frame {
                self.current_time += one_frame;
            }

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
                let texture = self.video.get_frame(self.current_time);
                let aspect_ratio = self.video.width() as f32 / self.video.height() as f32;

                view_builder.queue_draw(
                    RectangleDrawData::new(
                        re_ctx,
                        &[TexturedRect {
                            top_left_corner_position: [0.0; 3].into(),
                            extent_u: resolution[1] as f32 * aspect_ratio * glam::Vec3::X,
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
            let duration = self.video.duration_ms();

            let current_time = self.current_time;
            let new_time = match input.logical_key {
                Key::Named(NamedKey::ArrowRight) => current_time + APPROX_ONE_FRAME,
                Key::Named(NamedKey::ArrowLeft) => current_time - APPROX_ONE_FRAME,
                _ => current_time,
            };

            // handle 0, 1, 2, 3 keys etc. to seek to that fraction of the video duration
            match input.logical_key {
                Key::Character(v) if v.as_str() == "0" => self.current_time = 0.0,
                Key::Character(v) if v.as_str() == "1" => self.current_time = duration * 0.1,
                Key::Character(v) if v.as_str() == "2" => self.current_time = duration * 0.2,
                Key::Character(v) if v.as_str() == "3" => self.current_time = duration * 0.3,
                Key::Character(v) if v.as_str() == "4" => self.current_time = duration * 0.4,
                Key::Character(v) if v.as_str() == "5" => self.current_time = duration * 0.5,
                Key::Character(v) if v.as_str() == "6" => self.current_time = duration * 0.6,
                Key::Character(v) if v.as_str() == "7" => self.current_time = duration * 0.7,
                Key::Character(v) if v.as_str() == "8" => self.current_time = duration * 0.8,
                Key::Character(v) if v.as_str() == "9" => self.current_time = duration * 0.9,
                _ => {}
            }

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
