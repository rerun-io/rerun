//! Demonstrates the dedicated picking layer support.

use itertools::Itertools as _;
use rand::Rng as _;
use re_renderer::renderer::GpuMeshInstance;
use re_renderer::view_builder::{Projection, TargetConfiguration, ViewBuilder};
use re_renderer::{
    Color32, GpuReadbackIdentifier, PickingLayerId, PickingLayerInstanceId, PickingLayerProcessor,
    PointCloudBuilder, RectInt, Size, ViewPickingConfiguration,
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
    picking_position: glam::UVec2,
    model_mesh_instances: Vec<GpuMeshInstance>,
    mesh_is_hovered: bool,
}

fn random_color(rnd: &mut impl rand::Rng) -> Color32 {
    re_renderer::Hsva {
        h: rnd.random::<f32>(),
        s: rnd.random::<f32>() * 0.5 + 0.5,
        v: rnd.random::<f32>() * 0.5 + 0.5,
        a: 1.0,
    }
    .into()
}

/// Readback identifier for picking rects.
/// Identifiers don't need to be unique and we don't have anything interesting to distinguish here!
const READBACK_IDENTIFIER: GpuReadbackIdentifier = 0;

/// Mesh ID used for picking. Uses the entire 64bit range for testing.
const MESH_ID: PickingLayerId = PickingLayerId {
    object: re_renderer::PickingLayerObjectId(0x1234_5678_9012_3456),
    instance: re_renderer::PickingLayerInstanceId(0x3456_1234_5678_9012),
};

impl framework::Example for Picking {
    fn title() -> &'static str {
        "Picking"
    }

    fn on_cursor_moved(&mut self, position_in_pixel: glam::UVec2) {
        self.picking_position = position_in_pixel;
    }

    fn new(re_ctx: &re_renderer::RenderContext) -> Self {
        let mut rnd = <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(42);
        let random_point_range = -5.0_f32..5.0_f32;
        let point_count = 1000;

        // Split point cloud into several batches to test picking of multiple objects.
        let point_sets = (0..2)
            .map(|_| PointSet {
                positions: (0..point_count)
                    .map(|_| {
                        glam::vec3(
                            rnd.random_range(random_point_range.clone()),
                            rnd.random_range(random_point_range.clone()),
                            rnd.random_range(random_point_range.clone()),
                        )
                    })
                    .collect_vec(),
                radii: vec![Size::new_scene_units(0.08); point_count],
                colors: (0..point_count)
                    .map(|_| random_color(&mut rnd))
                    .collect_vec(),
                picking_ids: (0..point_count as u64)
                    .map(PickingLayerInstanceId)
                    .collect_vec(),
            })
            .collect_vec();

        let model_mesh_instances =
            crate::framework::load_rerun_mesh(re_ctx).expect("Failed to load rerun mesh");

        Self {
            point_sets,
            model_mesh_instances,
            picking_position: glam::UVec2::ZERO,
            mesh_is_hovered: false,
        }
    }

    fn draw(
        &mut self,
        re_ctx: &re_renderer::RenderContext,
        resolution: [u32; 2],
        _time: &framework::Time,
        pixels_per_point: f32,
    ) -> anyhow::Result<Vec<framework::ViewDrawResult>> {
        if let Some(picking_result) =
            PickingLayerProcessor::readback_result(re_ctx, READBACK_IDENTIFIER)
        {
            // Grab the middle pixel. usually we'd want to do something clever that snaps the closest object of interest.
            let picked_id = picking_result.picked_id(picking_result.rect.extent / 2);

            self.mesh_is_hovered = false;
            if picked_id == MESH_ID {
                self.mesh_is_hovered = true;
            } else if picked_id.object.0 != 0 && picked_id.object.0 <= self.point_sets.len() as u64
            {
                let point_set = &mut self.point_sets[picked_id.object.0 as usize - 1];
                point_set.radii[picked_id.instance.0 as usize] = Size::new_scene_units(0.1);
                point_set.colors[picked_id.instance.0 as usize] = Color32::DEBUG_COLOR;
            }
        }

        // TODO(#1426): unify camera logic between examples.
        let camera_position = glam::vec3(1.0, 3.5, 7.0);

        // Use an uneven number of pixels for the picking rect so that there is a clearly defined middle-pixel.
        // (for this sample a size of 1 would be sufficient, but for a real application you'd want to use a larger size to allow snapping)
        let picking_rect_size = 31;
        let picking_config = ViewPickingConfiguration {
            picking_rect: RectInt::from_middle_and_extent(
                self.picking_position.as_ivec2(),
                glam::uvec2(picking_rect_size, picking_rect_size),
            ),
            readback_identifier: READBACK_IDENTIFIER,
            show_debug_view: false,
        };

        let mut view_builder = ViewBuilder::new(
            re_ctx,
            TargetConfiguration {
                name: "OutlinesDemo".into(),
                resolution_in_pixel: resolution,
                view_from_world: macaw::IsoTransform::look_at_rh(
                    camera_position,
                    glam::Vec3::ZERO,
                    glam::Vec3::Y,
                )
                .ok_or_else(|| anyhow::format_err!("invalid camera"))?,
                projection_from_view: Projection::Perspective {
                    vertical_fov: 70.0 * std::f32::consts::TAU / 360.0,
                    near_plane_distance: 0.01,
                    aspect_ratio: resolution[0] as f32 / resolution[1] as f32,
                },
                pixels_per_point,
                outline_config: None,
                picking_config: Some(picking_config),
                ..Default::default()
            },
        )?;

        let mut point_builder = PointCloudBuilder::new(re_ctx);
        point_builder.reserve(self.point_sets.iter().map(|set| set.positions.len()).sum())?;
        for (i, point_set) in self.point_sets.iter().enumerate() {
            point_builder
                .batch(format!("Random Points {i}"))
                .picking_object_id(re_renderer::PickingLayerObjectId(i as u64 + 1)) // offset by one since 0=default=no hit
                .add_points(
                    &point_set.positions,
                    &point_set.radii,
                    &point_set.colors,
                    &point_set.picking_ids,
                );
        }
        view_builder.queue_draw(re_ctx, point_builder.into_draw_data()?);

        let instances = self
            .model_mesh_instances
            .iter()
            .map(|instance| GpuMeshInstance {
                gpu_mesh: instance.gpu_mesh.clone(),
                world_from_mesh: glam::Affine3A::from_translation(glam::vec3(0.0, 0.0, 0.0)),
                picking_layer_id: MESH_ID,
                additive_tint: if self.mesh_is_hovered {
                    Color32::DEBUG_COLOR
                } else {
                    Color32::BLACK
                },
                outline_mask_ids: Default::default(),
            })
            .collect_vec();

        view_builder.queue_draw(
            re_ctx,
            re_renderer::renderer::GenericSkyboxDrawData::new(re_ctx, Default::default()),
        );
        view_builder.queue_draw(
            re_ctx,
            re_renderer::renderer::MeshDrawData::new(re_ctx, &instances)?,
        );

        let command_buffer = view_builder.draw(re_ctx, re_renderer::Rgba::TRANSPARENT)?;

        Ok(vec![framework::ViewDrawResult {
            view_builder,
            command_buffer,
            target_location: glam::Vec2::ZERO,
        }])
    }

    fn on_key_event(&mut self, _input: winit::event::KeyEvent) {}
}

fn main() {
    framework::start::<Picking>();
}
