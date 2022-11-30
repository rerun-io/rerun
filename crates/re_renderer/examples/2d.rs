use re_renderer::{
    renderer::{
        LineDrawable, LineStrip, LineStripFlags, Rectangle, RectangleDrawData, TextureFilter,
    },
    resource_managers::{ResourceLifeTime, Texture2D, Texture2DHandle},
    view_builder::{self, ViewBuilder},
};

use smallvec::smallvec;

mod framework;

struct Render2D {
    rerun_logo_texture: Texture2DHandle,
    rerun_logo_texture_width: u32,
    rerun_logo_texture_height: u32,
}

impl framework::Example for Render2D {
    fn title() -> &'static str {
        "2D Rendering"
    }

    fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        let rerun_logo =
            image::load_from_memory(include_bytes!("../../re_viewer/data/logo_dark_mode.png"))
                .unwrap();

        let mut image_data = rerun_logo.as_rgba8().unwrap().to_vec();
        // Premultiply alpha (not doing any alpha blending, so this will look better on a black ground)
        for color in image_data.chunks_exact_mut(4) {
            let alpha = color[3] as f32 / 255.0;
            color[0] = (color[0] as f32 * alpha) as u8;
            color[1] = (color[1] as f32 * alpha) as u8;
            color[2] = (color[2] as f32 * alpha) as u8;
        }

        let rerun_logo_texture = re_ctx.texture_manager_2d.store_resource(
            Texture2D {
                label: "rerun logo".into(),
                data: image_data,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                width: rerun_logo.width(),
                height: rerun_logo.height(),
            },
            ResourceLifeTime::LongLived,
        );
        Render2D {
            rerun_logo_texture,

            rerun_logo_texture_width: rerun_logo.width(),
            rerun_logo_texture_height: rerun_logo.height(),
        }
    }

    fn draw(
        &mut self,
        re_ctx: &mut re_renderer::RenderContext,
        surface_configuration: &wgpu::SurfaceConfiguration,
        _time: &framework::Time,
    ) -> Vec<framework::ViewDrawResult> {
        let mut view_builder = ViewBuilder::default();
        view_builder
            .setup_view(
                re_ctx,
                view_builder::TargetConfiguration::new_2d_target(
                    "2D".into(),
                    [surface_configuration.width, surface_configuration.height],
                    1.0,
                ),
            )
            .unwrap();

        let screen_size = glam::vec2(
            surface_configuration.width as f32,
            surface_configuration.height as f32,
        );

        let line_radius = 5.0;
        let line_drawable = LineDrawable::new(
            re_ctx,
            &[
                // Green lines filling border
                LineStrip {
                    points: smallvec![
                        glam::vec3(line_radius, line_radius, 0.0),
                        glam::vec3(screen_size.x - line_radius, line_radius, 0.0),
                        glam::vec3(
                            screen_size.x - line_radius,
                            screen_size.y - line_radius,
                            0.0
                        ),
                        glam::vec3(line_radius, screen_size.y - line_radius, 0.0),
                        glam::vec3(line_radius, line_radius, 0.0),
                    ],
                    radius: line_radius,
                    color: [50, 255, 50, 255],
                    flags: LineStripFlags::empty(),
                },
                // Blue lines around the top left quarter.
                // TODO(andreas): This lines should be on top now, but they're below (for me at least, surprised there is no z-fighting)
                LineStrip {
                    points: smallvec![
                        glam::vec3(line_radius, line_radius, 0.0),
                        glam::vec3(screen_size.x * 0.5 - line_radius, line_radius, 0.0),
                        glam::vec3(
                            screen_size.x * 0.5 - line_radius,
                            screen_size.y * 0.5 - line_radius,
                            0.0
                        ),
                        glam::vec3(line_radius, screen_size.y * 0.5 - line_radius, 0.0),
                        glam::vec3(line_radius, line_radius, 0.0),
                    ],
                    radius: line_radius,
                    color: [50, 50, 255, 255],
                    flags: LineStripFlags::empty(),
                },
            ],
        )
        .unwrap();

        let image_scale = 8.0;
        let rectangle_draw_data = RectangleDrawData::new(
            re_ctx,
            &[
                Rectangle {
                    top_left_corner_position: glam::vec3(100.0, 100.0, -0.05),
                    extent_u: glam::vec3(self.rerun_logo_texture_width as f32, 0.0, 0.0)
                        * image_scale,
                    extent_v: glam::vec3(0.0, self.rerun_logo_texture_height as f32, 0.0)
                        * image_scale,
                    texture: self.rerun_logo_texture,
                    texture_filter: TextureFilter::Nearest,
                },
                Rectangle {
                    top_left_corner_position: glam::vec3(
                        100.0,
                        150.0 + self.rerun_logo_texture_height as f32 * image_scale,
                        -0.05,
                    ),
                    extent_u: glam::vec3(self.rerun_logo_texture_width as f32, 0.0, 0.0)
                        * image_scale,
                    extent_v: glam::vec3(0.0, self.rerun_logo_texture_height as f32, 0.0)
                        * image_scale,
                    texture: self.rerun_logo_texture,
                    texture_filter: TextureFilter::LinearNoMipMapping,
                },
            ],
        )
        .unwrap();

        view_builder.queue_draw(&line_drawable);
        view_builder.queue_draw(&rectangle_draw_data);

        let command_buffer = view_builder.draw(re_ctx).unwrap();

        vec![framework::ViewDrawResult {
            view_builder,
            command_buffer,
            target_location: glam::Vec2::ZERO,
        }]
    }

    fn on_keyboard_input(&mut self, _input: winit::event::KeyboardInput) {}
}

fn main() {
    framework::start::<Render2D>();
}
