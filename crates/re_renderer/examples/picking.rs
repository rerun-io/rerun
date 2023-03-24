use itertools::Itertools as _;
use rand::Rng;
use re_renderer::{
    view_builder::{Projection, TargetConfiguration, ViewBuilder},
    Color32, PointCloudBuilder, RenderContext, Size,
};

mod framework;

struct Picking {
    random_points_positions: Vec<glam::Vec3>,
    random_points_radii: Vec<Size>,
    random_points_colors: Vec<Color32>,
}

fn random_color(rnd: &mut impl rand::Rng) -> Color32 {
    ecolor::Hsva {
        h: rnd.gen::<f32>(),
        s: rnd.gen::<f32>() * 0.5 + 0.5,
        v: rnd.gen::<f32>() * 0.5 + 0.5,
        a: 1.0,
    }
    .into()
}

impl Picking {
    fn handle_incoming_picking_data(&mut self, re_ctx: &mut RenderContext) {
        re_ctx
            .gpu_readback_belt
            .lock()
            .receive_data(|_data, _identifier| {
                // TODO.
            });
    }
}

impl framework::Example for Picking {
    fn title() -> &'static str {
        "Picking"
    }

    fn new(_re_ctx: &mut re_renderer::RenderContext) -> Self {
        let mut rnd = <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(42);
        let random_point_range = -5.0_f32..5.0_f32;
        let point_count = 100000;
        let random_points_positions = (0..point_count)
            .map(|_| {
                glam::vec3(
                    rnd.gen_range(random_point_range.clone()),
                    rnd.gen_range(random_point_range.clone()),
                    rnd.gen_range(random_point_range.clone()),
                )
            })
            .collect_vec();
        let random_points_radii = (0..point_count)
            .map(|_| Size::new_scene(rnd.gen_range(0.005..0.05)))
            .collect_vec();
        let random_points_colors = (0..point_count)
            .map(|_| random_color(&mut rnd))
            .collect_vec();
        Picking {
            random_points_positions,
            random_points_radii,
            random_points_colors,
        }
    }

    fn draw(
        &mut self,
        re_ctx: &mut re_renderer::RenderContext,
        resolution: [u32; 2],
        _time: &framework::Time,
        pixels_from_point: f32,
    ) -> Vec<framework::ViewDrawResult> {
        self.handle_incoming_picking_data(re_ctx);

        let mut view_builder = ViewBuilder::default();

        // TODO(#1426): unify camera logic between examples.
        let camera_position = glam::vec3(1.0, 3.5, 7.0);

        view_builder
            .setup_view(
                re_ctx,
                TargetConfiguration {
                    name: "OutlinesDemo".into(),
                    resolution_in_pixel: resolution,
                    view_from_world: macaw::IsoTransform::look_at_rh(
                        camera_position,
                        glam::Vec3::ZERO,
                        glam::Vec3::Y,
                    )
                    .unwrap(),
                    projection_from_view: Projection::Perspective {
                        vertical_fov: 70.0 * std::f32::consts::TAU / 360.0,
                        near_plane_distance: 0.01,
                    },
                    pixels_from_point,
                    outline_config: None,
                    ..Default::default()
                },
            )
            .unwrap();

        view_builder.schedule_picking_readback(re_ctx, glam::uvec2(100, 100), 256, true);

        let mut builder = PointCloudBuilder::<()>::new(re_ctx);
        builder
            .batch("Random Points")
            .add_points(
                self.random_points_positions.len(),
                self.random_points_positions.iter().cloned(),
            )
            .radii(self.random_points_radii.iter().cloned())
            .colors(self.random_points_colors.iter().cloned());

        view_builder.queue_draw(&builder.to_draw_data(re_ctx).unwrap());
        view_builder.queue_draw(&re_renderer::renderer::GenericSkyboxDrawData::new(re_ctx));

        let command_buffer = view_builder
            .draw(re_ctx, ecolor::Rgba::TRANSPARENT)
            .unwrap();

        vec![framework::ViewDrawResult {
            view_builder,
            command_buffer,
            target_location: glam::Vec2::ZERO,
        }]
    }

    fn on_keyboard_input(&mut self, _input: winit::event::KeyboardInput) {}
}

fn main() {
    framework::start::<Picking>();
}
