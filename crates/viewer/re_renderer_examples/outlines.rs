//! Demonstrates outline rendering.

use itertools::Itertools as _;
use re_renderer::renderer::GpuMeshInstance;
use re_renderer::view_builder::{Projection, TargetConfiguration, ViewBuilder};
use re_renderer::{Color32, OutlineConfig, OutlineMaskPreference};
use winit::event::ElementState;

mod framework;

struct Outlines {
    is_paused: bool,
    secs_since_startup: f32,
    model_mesh_instances: Vec<GpuMeshInstance>,
}

struct MeshProperties {
    outline_mask_ids: OutlineMaskPreference,
    position: glam::Vec3,
    rotation: glam::Quat,
}

impl framework::Example for Outlines {
    fn title() -> &'static str {
        "Outlines"
    }

    fn new(re_ctx: &re_renderer::RenderContext) -> Self {
        Self {
            is_paused: false,
            secs_since_startup: 0.0,
            model_mesh_instances: crate::framework::load_rerun_mesh(re_ctx)
                .expect("Failed to load rerun mesh"),
        }
    }

    fn draw(
        &mut self,
        re_ctx: &re_renderer::RenderContext,
        resolution: [u32; 2],
        time: &framework::Time,
        pixels_per_point: f32,
    ) -> anyhow::Result<Vec<framework::ViewDrawResult>> {
        if !self.is_paused {
            self.secs_since_startup += time.last_frame_duration.as_secs_f32();
        }
        let secs_since_startup = self.secs_since_startup;
        // TODO(#1426): unify camera logic between examples.
        let camera_position = glam::vec3(1.0, 3.5, 7.0);

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
                #[expect(clippy::disallowed_methods)] // Hardcoded colors for this example.
                outline_config: Some(OutlineConfig {
                    outline_radius_pixel: (secs_since_startup * 2.0).sin().abs() * 10.0 + 2.0,
                    color_layer_a: re_renderer::Rgba::from_rgb(1.0, 0.6, 0.0),
                    color_layer_b: re_renderer::Rgba::from_rgba_unmultiplied(0.25, 0.3, 1.0, 0.5),
                }),
                ..Default::default()
            },
        )?;

        let outline_mask_large_mesh = match ((secs_since_startup * 0.5) as u64) % 5 {
            0 => OutlineMaskPreference::NONE,
            1 => OutlineMaskPreference::some(1, 0), // Same as the y spinning mesh.
            2 => OutlineMaskPreference::some(2, 0), // Different than both meshes, outline A.
            3 => OutlineMaskPreference::some(0, 1), // Same as the x spinning mesh.
            4 => OutlineMaskPreference::some(0, 2), // Different than both meshes, outline B.
            _ => unreachable!(),
        };

        let mesh_properties = [
            MeshProperties {
                outline_mask_ids: outline_mask_large_mesh,
                position: glam::Vec3::ZERO,
                rotation: glam::Quat::IDENTITY,
            },
            MeshProperties {
                outline_mask_ids: OutlineMaskPreference::some(1, 0),
                position: glam::vec3(2.0, 0.0, -3.0),
                rotation: glam::Quat::from_rotation_y(secs_since_startup),
            },
            MeshProperties {
                outline_mask_ids: OutlineMaskPreference::some(0, 1),
                position: glam::vec3(-2.0, 1.0, 3.0),
                rotation: glam::Quat::from_rotation_x(secs_since_startup),
            },
        ];

        let instances = mesh_properties
            .into_iter()
            .flat_map(|props| {
                self.model_mesh_instances
                    .iter()
                    .map(move |instance| GpuMeshInstance {
                        gpu_mesh: instance.gpu_mesh.clone(),
                        world_from_mesh: glam::Affine3A::from_rotation_translation(
                            props.rotation,
                            props.position,
                        ) * instance.world_from_mesh,
                        outline_mask_ids: props.outline_mask_ids,
                        picking_layer_id: Default::default(),
                        additive_tint: Color32::BLACK,
                    })
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

    fn on_key_event(&mut self, input: winit::event::KeyEvent) {
        if input.state == ElementState::Pressed
            && input.logical_key == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Space)
        {
            self.is_paused ^= true;
        }
    }
}

fn main() {
    framework::start::<Outlines>();
}
