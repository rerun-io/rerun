use re_renderer::view_builder::{self, ViewBuilder};

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
