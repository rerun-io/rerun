use re_renderer::{
    renderer::{
        LineDrawable, LineStrip, LineStripFlags, Rectangle, RectangleDrawData, TextureFilter,
    },
    resource_managers::{Texture2D, Texture2DHandle},
    view_builder::{self, ViewBuilder},
};

use smallvec::smallvec;

mod framework;

struct Render2D {
    rerun_logo_texture: Texture2DHandle,
}

impl framework::Example for Render2D {
    fn title() -> &'static str {
        "2D Rendering"
    }

    fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        let rerun_logo_texture = re_ctx.texture_manager_2d.white_texture();
        Render2D { rerun_logo_texture }
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

        let rectangle_draw_data = RectangleDrawData::new(
            re_ctx,
            &[Rectangle {
                top_left_corner_position: (screen_size * 0.25).extend(-0.05),
                extent_u: glam::vec3(screen_size.x * 0.5, 0.0, 0.0),
                extent_v: glam::vec3(0.0, screen_size.y * 0.5, 0.0),
                texture: self.rerun_logo_texture,
                texture_filter: TextureFilter::Nearest,
            }],
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
