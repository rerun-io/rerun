use ahash::HashMap;
use itertools::Itertools as _;
use rand::Rng;
use re_renderer::{
    view_builder::{Projection, TargetConfiguration, ViewBuilder},
    Color32, GpuReadbackBufferIdentifier, IntRect, PickingLayerId, PickingLayerInstanceId,
    PointCloudBuilder, RenderContext, ScheduledPickingRect, Size,
};

mod framework;

struct PointSet {
    positions: Vec<glam::Vec3>,
    radii: Vec<Size>,
    colors: Vec<Color32>,
    picking_ids: Vec<PickingLayerInstanceId>,
}

struct Picking {
    point_sets: Vec<PointSet>,
    scheduled_picking_rects: HashMap<GpuReadbackBufferIdentifier, ScheduledPickingRect>,
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
                if let Some(picking_rect_info) = self.scheduled_picking_rects.remove(&identifier) {
                    // TODO(andreas): Move this into a utility function?
                    let picking_data_without_padding =
                        picking_rect_info.row_info.remove_padding(data);
                    let picking_data: &[PickingLayerId] =
                        bytemuck::cast_slice(&picking_data_without_padding);

                    // Grab the middle pixel. usually we'd want to do something clever that snaps the the closest object of interest.
                    let picked_pixel = picking_data[(picking_rect_info.rect.extent.x / 2
                        + (picking_rect_info.rect.extent.y / 2) * picking_rect_info.rect.extent.x)
                        as usize];

                    if picked_pixel.object.0 != 0 {
                        let point_set = &mut self.point_sets[picked_pixel.object.0 as usize - 1];
                        point_set.radii[picked_pixel.instance.0 as usize] = Size::new_scene(0.1);
                        point_set.colors[picked_pixel.instance.0 as usize] = Color32::DEBUG_COLOR;
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

        // Split point cloud into several batches to test picking of multiple objects.
        let point_sets = (0..2)
            .map(|_| PointSet {
                positions: (0..point_count)
                    .map(|_| {
                        glam::vec3(
                            rnd.gen_range(random_point_range.clone()),
                            rnd.gen_range(random_point_range.clone()),
                            rnd.gen_range(random_point_range.clone()),
                        )
                    })
                    .collect_vec(),
                radii: std::iter::repeat(Size::new_scene(0.08))
                    .take(point_count)
                    .collect_vec(),
                colors: (0..point_count)
                    .map(|_| random_color(&mut rnd))
                    .collect_vec(),
                picking_ids: (0..point_count as u64)
                    .map(PickingLayerInstanceId)
                    .collect_vec(),
            })
            .collect_vec();

        Picking {
            point_sets,
            scheduled_picking_rects: HashMap::default(),
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
        let picking_rect = view_builder
            .schedule_picking_readback(
                re_ctx,
                IntRect {
                    top_left_corner: self.picking_position.as_ivec2()
                        - glam::ivec2(picking_rect_size / 2, picking_rect_size / 2),
                    extent: glam::ivec2(picking_rect_size, picking_rect_size).as_uvec2(),
                },
                false,
            )
            .unwrap();
        self.scheduled_picking_rects
            .insert(picking_rect.identifier, picking_rect);

        let mut builder = PointCloudBuilder::<()>::new(re_ctx);

        for (i, point_set) in self.point_sets.iter().enumerate() {
            builder
                .batch(format!("Random Points {i}"))
                .picking_object_id(re_renderer::PickingLayerObjectId(i as u64 + 1)) // offset by one since 0=default=no hit
                .add_points(
                    point_set.positions.len(),
                    point_set.positions.iter().cloned(),
                )
                .radii(point_set.radii.iter().cloned())
                .colors(point_set.colors.iter().cloned())
                .picking_instance_ids(point_set.picking_ids.iter().cloned());
        }
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
