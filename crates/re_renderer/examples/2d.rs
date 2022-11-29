use re_renderer::view_builder::{self, Projection, ViewBuilder};

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
    ) -> Vec<(ViewBuilder, wgpu::CommandBuffer)> {
        let mut view_builder = ViewBuilder::default();
        view_builder
            .setup_view(
                re_ctx,
                view_builder::TargetConfiguration {
                    name: "2D".into(),
                    resolution_in_pixel: [
                        surface_configuration.width,
                        surface_configuration.height,
                    ],
                    origin_in_pixel: [0, 0],
                    view_from_world: macaw::IsoTransform::IDENTITY,
                    projection_from_view: Projection::Orthographic {
                        vertical_world_size: surface_configuration.height as f32,
                        far_plane_distance: 100.0,
                    },
                },
            )
            .unwrap();

        let cmd_buf = view_builder.draw(re_ctx).unwrap();

        vec![(view_builder, cmd_buf)]
    }

    fn on_keyboard_input(&mut self, _input: winit::event::KeyboardInput) {}
}

fn main() {
    framework::start::<Render2D>();
}
