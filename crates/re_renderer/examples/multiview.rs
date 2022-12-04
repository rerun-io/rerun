use std::{f32::consts::TAU, io::Read};

use framework::Example;
use glam::Vec3;
use itertools::Itertools;
use macaw::IsoTransform;
use rand::Rng;

use re_renderer::{
    renderer::{
        GenericSkyboxDrawData, LineDrawData, LineStripFlags, MeshDrawData, MeshInstance,
        PointCloudDrawData, PointCloudPoint, TestTriangleDrawData,
    },
    resource_managers::ResourceLifeTime,
    texture_values::ValueRgba8UnormSrgb,
    view_builder::{OrthographicCameraMode, Projection, TargetConfiguration, ViewBuilder},
    LineStripSeriesBuilder, RenderContext,
};
use winit::event::{ElementState, VirtualKeyCode};

mod framework;

fn draw_view<'a, D: 'static + re_renderer::renderer::DrawData + Sync + Send + Clone>(
    re_ctx: &'a mut RenderContext,
    target_cfg: TargetConfiguration,
    skybox: &GenericSkyboxDrawData,
    draw_data: &D,
) -> (ViewBuilder, wgpu::CommandBuffer) {
    let mut view_builder = ViewBuilder::default();
    let command_buffer = view_builder
        .setup_view(re_ctx, target_cfg)
        .unwrap()
        .queue_draw(skybox)
        .queue_draw(draw_data)
        .draw(re_ctx, ValueRgba8UnormSrgb::TRANSPARENT)
        .unwrap();

    (view_builder, command_buffer)
}

fn build_mesh_instances(
    re_ctx: &mut RenderContext,
    model_mesh_instances: &[MeshInstance],
    mesh_instance_positions_and_colors: &[(glam::Vec3, [u8; 4])],
    seconds_since_startup: f32,
) -> MeshDrawData {
    let mesh_instances = mesh_instance_positions_and_colors
        .chunks_exact(model_mesh_instances.len())
        .enumerate()
        .flat_map(|(i, positions_and_colors)| {
            model_mesh_instances.iter().zip(positions_and_colors).map(
                move |(model_mesh_instances, (p, c))| MeshInstance {
                    gpu_mesh: model_mesh_instances.gpu_mesh.clone(),
                    mesh: None,
                    world_from_mesh: macaw::Conformal3::from_scale_rotation_translation(
                        0.025 + (i % 10) as f32 * 0.01,
                        glam::Quat::from_rotation_y(i as f32 + seconds_since_startup * 5.0),
                        *p,
                    ) * model_mesh_instances.world_from_mesh,
                    additive_tint_srgb: *c,
                },
            )
        })
        .collect_vec();
    MeshDrawData::new(re_ctx, &mesh_instances).unwrap()
}

fn lorenz_points(seconds_since_startup: f32) -> Vec<glam::Vec3> {
    // Lorenz attractor https://en.wikipedia.org/wiki/Lorenz_system
    fn lorenz_integrate(cur: glam::Vec3, dt: f32) -> glam::Vec3 {
        let sigma: f32 = 10.0;
        let rho: f32 = 28.0;
        let beta: f32 = 8.0 / 3.0;

        cur + glam::vec3(
            sigma * (cur.y - cur.x),
            cur.x * (rho - cur.z) - cur.y,
            cur.x * cur.y - beta * cur.z,
        ) * dt
    }

    // slow buildup and reset
    let num_points = (((seconds_since_startup * 0.05).fract() * 10000.0) as u32).max(1);

    let mut latest_point = glam::vec3(-0.1, 0.001, 0.0);
    std::iter::repeat_with(move || {
        latest_point = lorenz_integrate(latest_point, 0.005);
        latest_point
    })
    // lorenz system is sensitive to start conditions (.. that's the whole point), so transform after the fact
    .map(|p| (p + glam::vec3(-5.0, 0.0, -23.0)) * 0.6)
    .take(num_points as _)
    .collect()
}

fn build_lines(re_ctx: &mut RenderContext, seconds_since_startup: f32) -> LineDrawData {
    // Calculate some points that look nice for an animated line.
    let lorenz_points = lorenz_points(seconds_since_startup);

    let mut builder = LineStripSeriesBuilder::<()>::default();

    // Complex orange line.
    builder
        .add_strip(lorenz_points.into_iter())
        .color_rgb(255, 191, 0)
        .radius(0.05);

    // Green Zig-Zag arrow
    builder
        .add_strip(
            [
                glam::vec3(0.0, -1.0, 0.0),
                glam::vec3(1.0, 0.0, 0.0),
                glam::vec3(2.0, -1.0, 0.0),
                glam::vec3(3.0, 0.0, 0.0),
            ]
            .into_iter(),
        )
        .color_rgb(50, 255, 50)
        .radius(0.05)
        .flags(LineStripFlags::CAP_END_TRIANGLE);

    // Blue spiral
    builder
        .add_strip((0..1000).map(|i| {
            glam::vec3(
                (i as f32 * 0.01).sin() * 2.0,
                i as f32 * 0.01 - 6.0,
                (i as f32 * 0.01).cos() * 2.0,
            )
        }))
        .color_rgb(50, 50, 255)
        .radius(0.1)
        .flags(LineStripFlags::CAP_END_TRIANGLE);

    builder.to_draw_data(re_ctx)
}

enum CameraControl {
    RotateAroundCenter,

    // TODO(andreas): Only pauses rotation right now. Add camera controller.
    Manual,
}

struct Multiview {
    perspective_projection: bool,

    camera_control: CameraControl,
    camera_position: glam::Vec3,

    model_mesh_instances: Vec<MeshInstance>,
    mesh_instance_positions_and_colors: Vec<(glam::Vec3, [u8; 4])>,

    // Want to have a large cloud of random points, but doing rng for all of them every frame is too slow
    random_points: Vec<PointCloudPoint>,
}

impl Example for Multiview {
    fn title() -> &'static str {
        "Multiple Views"
    }

    fn new(re_ctx: &mut RenderContext) -> Self {
        re_log::info!("Switch between orthographic & perspective by pressing 'O'");
        re_log::info!("Stop camera movement by pressing 'Space'");

        let mut rnd = <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(42);
        let random_point_range = -5.0_f32..5.0_f32;
        let random_points = (0..500000)
            .map(|_| PointCloudPoint {
                position: glam::vec3(
                    rnd.gen_range(random_point_range.clone()),
                    rnd.gen_range(random_point_range.clone()),
                    rnd.gen_range(random_point_range.clone()),
                ),
                radius: rnd.gen_range(0.005..0.05),
                srgb_color: [rnd.gen(), rnd.gen(), rnd.gen(), 255],
            })
            .collect_vec();

        let model_mesh_instances = {
            let reader = std::io::Cursor::new(include_bytes!("rerun.obj.zip"));
            let mut zip = zip::ZipArchive::new(reader).unwrap();
            let mut zipped_obj = zip.by_name("rerun.obj").unwrap();
            let mut obj_data = Vec::new();
            zipped_obj.read_to_end(&mut obj_data).unwrap();
            re_renderer::importer::obj::load_obj_from_buffer(
                &obj_data,
                ResourceLifeTime::LongLived,
                re_ctx,
            )
            .unwrap()
        };

        let mesh_instance_positions_and_colors = lorenz_points(10.0)
            .iter()
            .flat_map(|p| {
                model_mesh_instances.iter().map(|_| {
                    let mut rnd = rand::thread_rng();
                    (*p, [rnd.gen(), rnd.gen(), rnd.gen(), 255])
                })
            })
            .collect();

        Self {
            perspective_projection: true,

            camera_control: CameraControl::RotateAroundCenter,
            camera_position: glam::Vec3::ZERO,

            model_mesh_instances,
            mesh_instance_positions_and_colors,
            random_points,
        }
    }

    fn draw(
        &mut self,
        re_ctx: &mut RenderContext,
        resolution: [u32; 2],
        time: &framework::Time,
    ) -> Vec<framework::ViewDrawResult> {
        if matches!(self.camera_control, CameraControl::RotateAroundCenter) {
            let seconds_since_startup = time.seconds_since_startup();
            self.camera_position = Vec3::new(
                seconds_since_startup.sin(),
                0.5,
                seconds_since_startup.cos(),
            ) * 10.0;
        }

        let seconds_since_startup = time.seconds_since_startup();
        let view_from_world =
            IsoTransform::look_at_rh(self.camera_position, Vec3::ZERO, Vec3::Y).unwrap();

        let triangle = TestTriangleDrawData::new(re_ctx);
        let skybox = GenericSkyboxDrawData::new(re_ctx);
        let lines = build_lines(re_ctx, seconds_since_startup);
        let point_cloud = PointCloudDrawData::new(re_ctx, &self.random_points).unwrap();
        let meshes = build_mesh_instances(
            re_ctx,
            &self.model_mesh_instances,
            &self.mesh_instance_positions_and_colors,
            seconds_since_startup,
        );

        let splits = framework::split_resolution(resolution, 2, 2).collect::<Vec<_>>();

        let projection_from_view = if self.perspective_projection {
            Projection::Perspective {
                vertical_fov: 70.0 * TAU / 360.0,
                near_plane_distance: 0.01,
            }
        } else {
            Projection::Orthographic {
                camera_mode: OrthographicCameraMode::NearPlaneCenter,
                vertical_world_size: 15.0,
                far_plane_distance: 100000.0,
            }
        };

        // Using a macro here because `DrawData` isn't object safe and a closure cannot be
        // generic over its input type.
        #[rustfmt::skip]
        macro_rules! draw {
            ($name:ident @ split #$n:expr) => {{
                let (view_builder, command_buffer) = draw_view(re_ctx,
                    TargetConfiguration {
                        name: stringify!($name).into(),
                        resolution_in_pixel: splits[$n].resolution_in_pixel,
                        view_from_world,
                        projection_from_view: projection_from_view.clone(),
                    },
                    &skybox,
                    &$name
                );
                framework::ViewDrawResult {
                    view_builder,
                    command_buffer,
                    target_location: splits[$n].target_location,
                }
            }};
        }

        vec![
            draw!(triangle @ split #0),
            draw!(lines @ split #1),
            draw!(meshes @ split #2),
            draw!(point_cloud @ split #3),
        ]
    }

    fn on_keyboard_input(&mut self, input: winit::event::KeyboardInput) {
        match (input.state, input.virtual_keycode) {
            (ElementState::Pressed, Some(VirtualKeyCode::O)) => {
                self.perspective_projection = !self.perspective_projection;
            }

            (ElementState::Pressed, Some(VirtualKeyCode::Space)) => {
                self.camera_control = match self.camera_control {
                    CameraControl::RotateAroundCenter => CameraControl::Manual,
                    CameraControl::Manual => CameraControl::RotateAroundCenter,
                };
            }

            _ => {}
        }
    }
}

fn main() {
    framework::start::<Multiview>();
}
