use itertools::Itertools as _;
use rand::Rng;
use re_renderer::{
    view_builder::{Projection, TargetConfiguration, ViewBuilder},
    Color32, PickingLayerInstanceId, PointCloudBuilder, RenderContext, ScheduledPickingRect, Size,
};

mod framework;

struct Picking {
    random_points_positions: Vec<glam::Vec3>,
    random_points_radii: Vec<Size>,
    random_points_colors: Vec<Color32>,
    random_points_picking_ids: Vec<PickingLayerInstanceId>,

    scheduled_picking_rects: Vec<ScheduledPickingRect>,

    picking_position: glam::UVec2,
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
    #[allow(clippy::unused_self)]
    fn handle_incoming_picking_data(&mut self, re_ctx: &mut RenderContext) {
        re_ctx
            .gpu_readback_belt
            .lock()
            .receive_data(|_data, identifier| {
                if let Some(index) = self
                    .scheduled_picking_rects
                    .iter()
                    .position(|s| s.identifier == identifier)
                {
                    let picking_rect_info = self.scheduled_picking_rects.swap_remove(index);
                    // TODO(andreas): Process picking data
                    let _ = picking_rect_info;
                } else {
                    re_log::error!("Received picking data for unknown identifier");
                }
            });
    }
}

impl framework::Example for Picking {
    fn title() -> &'static str {
        "Picking"
    }

    fn on_cursor_moved(&mut self, position_in_pixel: glam::UVec2) {
        self.picking_position = position_in_pixel;
    }

    fn new(_re_ctx: &mut re_renderer::RenderContext) -> Self {
        let mut rnd = <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(42);
        let random_point_range = -5.0_f32..5.0_f32;
        let point_count = 1000;
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
            .map(|_| Size::new_scene(rnd.gen_range(0.05..0.1)))
            .collect_vec();
        let random_points_colors = (0..point_count)
            .map(|_| random_color(&mut rnd))
            .collect_vec();
        let random_points_picking_ids = (0..point_count)
            .map(|i| PickingLayerInstanceId([0, i]))
            .collect_vec();

        Picking {
            random_points_positions,
            random_points_radii,
            random_points_colors,
            random_points_picking_ids,
            scheduled_picking_rects: Vec::new(),
            picking_position: glam::UVec2::ZERO,
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

        let picking_rect_size = 128;
        self.scheduled_picking_rects.push(
            view_builder
                .schedule_picking_readback(
                    re_ctx,
                    self.picking_position.as_ivec2()
                        - glam::ivec2(picking_rect_size / 2, picking_rect_size / 2),
                    picking_rect_size as u32,
                    true,
                )
                .unwrap(),
        );

        let mut builder = PointCloudBuilder::<()>::new(re_ctx);
        builder
            .batch("Random Points 1")
            .picking_object_id(re_renderer::PickingLayerObjectId([0, 10]))
            .add_points(
                self.random_points_positions.len(),
                self.random_points_positions.iter().cloned(),
            )
            .radii(self.random_points_radii.iter().cloned())
            .colors(self.random_points_colors.iter().cloned())
            .picking_instance_ids(self.random_points_picking_ids.iter().cloned());
        builder
            .batch("Random Points 2")
            .picking_object_id(re_renderer::PickingLayerObjectId([10, 0]))
            .world_from_obj(glam::Mat4::from_rotation_x(0.5))
            .add_points(
                self.random_points_positions.len(),
                self.random_points_positions.iter().cloned(),
            )
            .radii(self.random_points_radii.iter().cloned())
            .colors(self.random_points_colors.iter().cloned())
            .picking_instance_ids(self.random_points_picking_ids.iter().cloned());

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
