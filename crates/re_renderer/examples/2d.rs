use re_renderer::{
    view_builder::{self, ViewBuilder},
    LineStripSeriesBuilder,
};

mod framework;

struct Render2D {}

impl framework::Example for Render2D {
    fn title() -> &'static str {
        "2D Rendering"
    }

    fn new(_re_ctx: &mut re_renderer::RenderContext) -> Self {
        Render2D {}
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

        let mut line_strip_builder = LineStripSeriesBuilder::default();
        // Green lines filling border
        line_strip_builder
            .add_strip_2d(
                [
                    glam::vec2(line_radius, line_radius),
                    glam::vec2(screen_size.x - line_radius, line_radius),
                    glam::vec2(screen_size.x - line_radius, screen_size.y - line_radius),
                    glam::vec2(line_radius, screen_size.y - line_radius),
                    glam::vec2(line_radius, line_radius),
                ]
                .into_iter(),
            )
            .radius(line_radius)
            .color_rgb(50, 255, 50);

        // Blue lines around the top left quarter.
        line_strip_builder
            .add_strip_2d(
                [
                    glam::vec2(line_radius, line_radius),
                    glam::vec2(screen_size.x * 0.5 - line_radius, line_radius),
                    glam::vec2(
                        screen_size.x * 0.5 - line_radius,
                        screen_size.y * 0.5 - line_radius,
                    ),
                    glam::vec2(line_radius, screen_size.y * 0.5 - line_radius),
                    glam::vec2(line_radius, line_radius),
                ]
                .into_iter(),
            )
            .radius(line_radius)
            .color_rgb(50, 50, 255);
        view_builder.queue_draw(&line_strip_builder.to_drawable(re_ctx));

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
