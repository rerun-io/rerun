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

        let seconds_since_startup = time.seconds_since_startup();
        // TODO(#1426): unify camera logic between examples.
        // let camera_position = glam::vec3(
        //     (seconds_since_startup * 0.5).sin() * 10.0,
        //     2.5,
        //     (seconds_since_startup * 0.5).cos() * 10.0,
        // );
        let camera_position = glam::vec3(1.0, 3.5, 7.0);

        view_builder
            .setup_view(
                re_ctx,
                TargetConfiguration {
                    name: "2D".into(),
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
                    outline_config: Some(OutlineConfig {
                        outline_radius_pixel: (seconds_since_startup * 2.0).sin().abs() * 10.0
                            + 2.0,
                        color_layer_a: re_renderer::Rgba::from_rgb(1.0, 0.6, 0.0),
                        color_layer_b: re_renderer::Rgba::from_rgba_unmultiplied(
                            0.25, 0.3, 1.0, 0.5,
                        ),
                    }),
                    ..Default::default()
                },
            )
            .unwrap();

        let outline_mask_large_mesh = match ((seconds_since_startup * 0.5) as u64) % 5 {
            0 => None,
            1 => Some(glam::uvec2(1, 0)), // Same as the the y spinning mesh.
            2 => Some(glam::uvec2(2, 0)), // Different than both meshes, outline A.
            3 => Some(glam::uvec2(0, 1)), // Same as the the x spinning mesh.
            4 => Some(glam::uvec2(0, 2)), // Different than both meshes, outline B.
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
                position: glam::vec3(2.0, 0.0, -3.0),
                rotation: glam::Quat::from_rotation_y(seconds_since_startup),
            },
            MeshProperties {
                outline_mask: Some(glam::uvec2(0, 1)),
                position: glam::vec3(-2.0, 1.0, 3.0),
                rotation: glam::Quat::from_rotation_x(seconds_since_startup),
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
