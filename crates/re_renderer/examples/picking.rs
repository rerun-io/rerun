use itertools::Itertools as _;
use rand::Rng;
use re_renderer::{
    view_builder::{Projection, TargetConfiguration, ViewBuilder},
    Color32, PickingLayerId, PickingLayerInstanceId, PointCloudBuilder, RenderContext,
    ScheduledPickingRect, Size,
};

mod framework;

struct Picking {
    points_positions: Vec<glam::Vec3>,
    points_radii: Vec<Size>,
    points_colors: Vec<Color32>,
    points_picking_ids: Vec<PickingLayerInstanceId>,

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
    fn handle_incoming_picking_data(&mut self, re_ctx: &mut RenderContext, _time: f32) {
        re_ctx
            .gpu_readback_belt
            .lock()
            .receive_data(|data, identifier| {
                if let Some(index) = self
                    .scheduled_picking_rects
                    .iter()
                    .position(|s| s.identifier == identifier)
                {
                    let picking_rect_info = self.scheduled_picking_rects.swap_remove(index);

                    // TODO(andreas): Move this into a utility function?
                    let picking_data_without_padding =
                        picking_rect_info.row_info.remove_padding(data);
                    let picking_data: &[PickingLayerId] =
                        bytemuck::cast_slice(&picking_data_without_padding);

                    // Grab the middle pixel. usually we'd want to do something clever that snaps the the closest object of interest.
                    let picked_pixel = picking_data[(picking_rect_info.extent.x / 2
                        + (picking_rect_info.extent.y / 2) * picking_rect_info.extent.x)
                        as usize];
                    if picked_pixel.object.0 != 0 {
                        let index = picked_pixel.instance.0
                            + (self.points_radii.len() as u64 / 2) * (picked_pixel.object.0 - 1);
                        self.points_radii[index as usize] = Size::new_scene(0.1);
                        self.points_colors[index as usize] = Color32::DEBUG_COLOR;
                    }
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
        let point_count = 10000;
        let points_positions = (0..point_count)
            .map(|_| {
                glam::vec3(
                    rnd.gen_range(random_point_range.clone()),
                    rnd.gen_range(random_point_range.clone()),
                    rnd.gen_range(random_point_range.clone()),
                )
            })
            .collect_vec();
        let points_radii = std::iter::repeat(Size::new_scene(0.08))
            .take(point_count)
            .collect_vec();
        let points_colors = (0..point_count)
            .map(|_| random_color(&mut rnd))
            .collect_vec();
        let points_picking_ids = (0..point_count as u64)
            .map(PickingLayerInstanceId)
            .collect_vec();

        Picking {
            points_positions,
            points_radii,
            points_colors,
            points_picking_ids,
            scheduled_picking_rects: Vec::new(),
            picking_position: glam::UVec2::ZERO,
        }
    }

    fn draw(
        &mut self,
        re_ctx: &mut re_renderer::RenderContext,
        resolution: [u32; 2],
        time: &framework::Time,
        pixels_from_point: f32,
    ) -> Vec<framework::ViewDrawResult> {
        self.handle_incoming_picking_data(re_ctx, time.seconds_since_startup());

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

        let picking_rect_size = 32;
        self.scheduled_picking_rects.push(
            view_builder
                .schedule_picking_readback(
                    re_ctx,
                    self.picking_position.as_ivec2()
                        - glam::ivec2(picking_rect_size / 2, picking_rect_size / 2),
                    picking_rect_size as u32,
                    false,
                )
                .unwrap(),
        );

        let mut builder = PointCloudBuilder::<()>::new(re_ctx);

        // Split into two batches to test picking of multiple batches.
        let num_per_batch = self.points_positions.len() / 2;
        builder
            .batch("Random Points 1")
            .picking_object_id(re_renderer::PickingLayerObjectId(1))
            .add_points(
                num_per_batch,
                self.points_positions.iter().take(num_per_batch).cloned(),
            )
            .radii(self.points_radii.iter().take(num_per_batch).cloned())
            .colors(self.points_colors.iter().take(num_per_batch).cloned())
            .picking_instance_ids(self.points_picking_ids.iter().cloned());
        builder
            .batch("Random Points 2")
            .picking_object_id(re_renderer::PickingLayerObjectId(2))
            .add_points(
                num_per_batch,
                self.points_positions.iter().skip(num_per_batch).cloned(),
            )
            .radii(self.points_radii.iter().skip(num_per_batch).cloned())
            .colors(self.points_colors.iter().skip(num_per_batch).cloned())
            .picking_instance_ids(self.points_picking_ids.iter().cloned());

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
