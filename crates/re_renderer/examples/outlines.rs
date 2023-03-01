use itertools::Itertools;
use re_renderer::{
    renderer::{MeshInstance, OutlineConfig},
    view_builder::{Projection, TargetConfiguration, ViewBuilder},
};

mod framework;

struct Outlines {
    model_mesh_instances: Vec<MeshInstance>,
}

impl framework::Example for Outlines {
    fn title() -> &'static str {
        "Outlines"
    }

    fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        Outlines {
            model_mesh_instances: crate::framework::load_rerun_mesh(re_ctx),
        }
    }

    fn draw(
        &mut self,
        re_ctx: &mut re_renderer::RenderContext,
        resolution: [u32; 2],
        time: &framework::Time,
        pixels_from_point: f32,
    ) -> Vec<framework::ViewDrawResult> {
        let mut view_builder = ViewBuilder::default();
        view_builder
            .setup_view(
                re_ctx,
                TargetConfiguration {
                    name: "2D".into(),
                    resolution_in_pixel: resolution,
                    view_from_world: macaw::IsoTransform::from_translation(glam::vec3(
                        0.0, -1.0, -6.0,
                    )),
                    projection_from_view: Projection::Perspective {
                        vertical_fov: 70.0 * std::f32::consts::TAU / 360.0,
                        near_plane_distance: 0.01,
                    },
                    pixels_from_point,
                    outline_config: Some(OutlineConfig {
                        color_layer_0: re_renderer::Rgba::RED,
                        color_layer_1: re_renderer::Rgba::BLUE,
                    }),
                    ..Default::default()
                },
            )
            .unwrap();

        let instances = (0..3)
            .into_iter()
            .flat_map(|i| {
                self.model_mesh_instances
                    .iter()
                    .map(move |instance| MeshInstance {
                        gpu_mesh: instance.gpu_mesh.clone(),
                        mesh: None,
                        world_from_mesh: glam::Affine3A::from_rotation_translation(
                            glam::Quat::from_rotation_y(time.seconds_since_startup() * i as f32),
                            glam::vec3(8.0, 0.5, -10.0) * i as f32,
                        ) * instance.world_from_mesh,
                        additive_tint: re_renderer::Color32::TRANSPARENT,
                        outline_mask: if i == 0 {
                            None
                        } else {
                            Some(glam::uvec2(i, 0))
                        },
                    })
            })
            .collect_vec();

        view_builder.queue_draw(&re_renderer::renderer::GenericSkyboxDrawData::new(re_ctx));
        view_builder
            .queue_draw(&re_renderer::renderer::MeshDrawData::new(re_ctx, &instances).unwrap());

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
    framework::start::<Outlines>();
}
