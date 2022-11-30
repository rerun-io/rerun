use re_renderer::{
    renderer::LineStripFlags,
    view_builder::{self, Projection, ViewBuilder},
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
        resolution: [u32; 2],
        time: &framework::Time,
    ) -> Vec<framework::ViewDrawResult> {
        let splits = framework::split_resolution(resolution, 1, 2).collect::<Vec<_>>();

        let screen_size = glam::vec2(
            splits[0].resolution_in_pixel[0] as f32,
            splits[0].resolution_in_pixel[1] as f32,
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

        // Red Zig-Zag arrow in the middle
        line_strip_builder
            .add_strip_2d(
                [
                    screen_size * 0.5 - screen_size * 0.25,
                    screen_size * 0.5 + glam::vec2(-screen_size.x * 0.125, screen_size.x * 0.25),
                    screen_size * 0.5 - glam::vec2(-screen_size.x * 0.125, screen_size.x * 0.25),
                    screen_size * 0.5 + screen_size * 0.25,
                ]
                .into_iter(),
            )
            .radius(line_radius)
            .color_rgb(255, 50, 50)
            .flags(LineStripFlags::CAP_END_TRIANGLE);

        vec![
            // 2d view to the left
            {
                let mut view_builder = ViewBuilder::default();
                view_builder
                    .setup_view(
                        re_ctx,
                        view_builder::TargetConfiguration::new_2d_target(
                            "2D".into(),
                            splits[0].resolution_in_pixel,
                            1.0,
                        ),
                    )
                    .unwrap();
                view_builder.queue_draw(&line_strip_builder.to_drawable(re_ctx));
                let command_buffer = view_builder.draw(re_ctx).unwrap();
                framework::ViewDrawResult {
                    view_builder,
                    command_buffer,
                    target_location: splits[0].target_location,
                }
            },
            // and 3d view of the same scene to the right
            {
                {
                    let mut view_builder = ViewBuilder::default();
                    let seconds_since_startup = time.seconds_since_startup();
                    let camera_rotation_center = screen_size.extend(0.0) * 0.5;
                    let camera_position = glam::vec3(
                        seconds_since_startup.sin(),
                        0.5,
                        seconds_since_startup.cos(),
                    ) * screen_size.x.max(screen_size.y)
                        + camera_rotation_center;
                    view_builder
                        .setup_view(
                            re_ctx,
                            view_builder::TargetConfiguration {
                                name: "3D".into(),
                                resolution_in_pixel: splits[1].resolution_in_pixel,
                                view_from_world: macaw::IsoTransform::look_at_rh(
                                    camera_position,
                                    camera_rotation_center,
                                    glam::Vec3::Y,
                                )
                                .unwrap(),
                                projection_from_view: Projection::Perspective {
                                    vertical_fov: 70.0 * std::f32::consts::TAU / 360.0,
                                    near_plane_distance: 0.01,
                                },
                            },
                        )
                        .unwrap();
                    view_builder.queue_draw(&line_strip_builder.to_drawable(re_ctx));
                    let command_buffer = view_builder.draw(re_ctx).unwrap();
                    framework::ViewDrawResult {
                        view_builder,
                        command_buffer,
                        target_location: splits[1].target_location,
                    }
                }
            },
        ]
    }

    fn on_keyboard_input(&mut self, _input: winit::event::KeyboardInput) {}
}

fn main() {
    framework::start::<Render2D>();
}
