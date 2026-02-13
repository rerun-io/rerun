//! Demonstrates world grid rendering.
//!
//! Controls:
//! - Space: Pause
//! - G: Toggle camera mode
// TODO(#1426): unify camera logic between examples and add a free camera.

use re_renderer::renderer::GpuMeshInstance;
use re_renderer::view_builder::{Projection, TargetConfiguration, ViewBuilder};
use winit::event::ElementState;

mod framework;

enum CameraMode {
    ZoomInAndOut,
    Wobble,
}

struct Outlines {
    is_paused: bool,
    camera_mode: CameraMode,
    secs_since_startup: f32,
    model_mesh_instances: Vec<GpuMeshInstance>,
}

impl framework::Example for Outlines {
    fn title() -> &'static str {
        "world_grid"
    }

    fn new(re_ctx: &re_renderer::RenderContext) -> Self {
        Self {
            is_paused: false,
            camera_mode: CameraMode::Wobble,
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

        let camera_position = match self.camera_mode {
            CameraMode::ZoomInAndOut => {
                glam::vec3(1.0, 3.5, 7.0) * (secs_since_startup.sin() + 1.4)
            }
            CameraMode::Wobble => {
                glam::vec3(
                    secs_since_startup.sin(),
                    secs_since_startup.sin() * 0.5,
                    secs_since_startup.cos(),
                ) * 7.0
            }
        };

        let mut view_builder = ViewBuilder::new(
            re_ctx,
            TargetConfiguration {
                name: "WorldGridDemo".into(),
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
                ..Default::default()
            },
        )?;

        view_builder.queue_draw(
            re_ctx,
            re_renderer::renderer::GenericSkyboxDrawData::new(re_ctx, Default::default()),
        );
        view_builder.queue_draw(
            re_ctx,
            re_renderer::renderer::WorldGridDrawData::new(
                re_ctx,
                &re_renderer::renderer::WorldGridConfiguration {
                    #[expect(clippy::disallowed_methods)]
                    color: re_renderer::Rgba::from_rgb(0.5, 0.5, 0.5),
                    spacing: 0.1,
                    thickness_ui: 1.0,
                    plane: macaw::Plane3::ZX,
                },
            ),
        );
        view_builder.queue_draw(
            re_ctx,
            re_renderer::renderer::MeshDrawData::new(re_ctx, &self.model_mesh_instances)?,
        );

        let command_buffer = view_builder.draw(re_ctx, re_renderer::Rgba::TRANSPARENT)?;

        Ok(vec![framework::ViewDrawResult {
            view_builder,
            command_buffer,
            target_location: glam::Vec2::ZERO,
        }])
    }

    fn on_key_event(&mut self, input: winit::event::KeyEvent) {
        if input.state == ElementState::Pressed {
            if input.logical_key == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Space) {
                self.is_paused ^= true;
            } else if input.physical_key
                == winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyG)
            {
                self.camera_mode = match self.camera_mode {
                    CameraMode::ZoomInAndOut => CameraMode::Wobble,
                    CameraMode::Wobble => CameraMode::ZoomInAndOut,
                };
            }
        }
    }
}

fn main() {
    framework::start::<Outlines>();
}
