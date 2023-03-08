use itertools::Itertools;
use re_renderer::{
    renderer::{MeshInstance, OutlineConfig},
    view_builder::{Projection, TargetConfiguration, ViewBuilder},
};

mod framework;

struct Outlines {
    model_mesh_instances: Vec<MeshInstance>,
}

struct MeshProperties {
    outline_mask: Option<glam::UVec2>,
    position: glam::Vec3,
    rotation: glam::Quat,
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
                        outline_thickness_pixel: 32.0,
                        color_layer_0: re_renderer::Rgba::RED,
                        color_layer_1: re_renderer::Rgba::BLUE,
                    }),
                    ..Default::default()
                },
            )
            .unwrap();

        let outline_mask_large_mesh = match (time.seconds_since_startup() as u64) % 3 {
            0 => None,
            1 => Some(glam::uvec2(1, 0)), // Same as the other mesh.
            2 => Some(glam::uvec2(2, 0)), // Different from the other mesh, demonstrating that the outline is not shared.
            _ => unreachable!(),
        };

        let mesh_properties = [
            MeshProperties {
                outline_mask: outline_mask_large_mesh,
                position: glam::Vec3::ZERO,
                rotation: glam::Quat::IDENTITY,
            },
            MeshProperties {
                outline_mask: Some(glam::uvec2(1, 0)),
                position: glam::vec3(0.0, 1.0, -8.0),
                rotation: glam::Quat::from_rotation_y(time.seconds_since_startup()),
            },
        ];

        let instances = mesh_properties
            .into_iter()
            .flat_map(|props| {
                self.model_mesh_instances
                    .iter()
                    .map(move |instance| MeshInstance {
                        gpu_mesh: instance.gpu_mesh.clone(),
                        mesh: None,
                        world_from_mesh: glam::Affine3A::from_rotation_translation(
                            props.rotation,
                            props.position,
                        ) * instance.world_from_mesh,
                        outline_mask: props.outline_mask,
                        ..Default::default()
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
