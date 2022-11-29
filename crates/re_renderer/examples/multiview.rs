use std::{f32::consts::TAU, io::Read};

use framework::Example;
use glam::Vec3;
use itertools::Itertools;
use macaw::IsoTransform;
use rand::Rng;
use smallvec::smallvec;

use re_renderer::{
    renderer::{
        GenericSkyboxDrawable, LineDrawable, LineStrip, LineStripFlags, MeshDrawable, MeshInstance,
        PointCloudDrawable, PointCloudPoint, TestTriangleDrawable,
    },
    resource_managers::ResourceLifeTime,
    view_builder::{Projection, TargetConfiguration, ViewBuilder},
    RenderContext,
};
use winit::event::{ElementState, VirtualKeyCode};

mod framework;

fn split_resolution(
    resolution: [u32; 2],
    nb_rows: usize,
    nb_cols: usize,
) -> impl Iterator<Item = ((f32, f32), (f32, f32))> {
    let total_width = resolution[0] as f32;
    let total_height = resolution[1] as f32;
    let width = total_width / nb_cols as f32;
    let height = total_height / nb_rows as f32;
    (0..nb_rows)
        .flat_map(move |row| (0..nb_cols).map(move |col| (row, col)))
        .map(move |(row, col)| {
            // very quick'n'dirty (uneven) borders
            let y = f32::clamp(row as f32 * height + 2.0, 2.0, total_height - 2.0);
            let x = f32::clamp(col as f32 * width + 2.0, 2.0, total_width - 2.0);
            ((x, y), (width - 4.0, height - 4.0))
        })
}

fn draw_view<'a, D: 'static + re_renderer::renderer::Drawable + Sync + Send + Clone>(
    re_ctx: &'a mut RenderContext,
    target_cfg: TargetConfiguration,
    skybox: &GenericSkyboxDrawable,
    drawable: &D,
) -> (ViewBuilder, wgpu::CommandBuffer) {
    let mut view_builder = ViewBuilder::default();
    let command_buffer = view_builder
        .setup_view(re_ctx, target_cfg)
        .unwrap()
        .queue_draw(skybox)
        .queue_draw(drawable)
        .draw(re_ctx)
        .unwrap();

    (view_builder, command_buffer)
}

fn build_mesh_instances(
    re_ctx: &mut RenderContext,
    model_mesh_instances: &[MeshInstance],
    mesh_instance_positions_and_colors: &[(glam::Vec3, [u8; 4])],
    seconds_since_startup: f32,
) -> MeshDrawable {
    let mesh_instances = mesh_instance_positions_and_colors
        .chunks_exact(model_mesh_instances.len())
        .enumerate()
        .flat_map(|(i, positions_and_colors)| {
            model_mesh_instances.iter().zip(positions_and_colors).map(
                move |(model_mesh_instances, (p, c))| MeshInstance {
                    mesh: model_mesh_instances.mesh,
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
    MeshDrawable::new(re_ctx, &mesh_instances).unwrap()
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

fn build_lines(re_ctx: &mut RenderContext, seconds_since_startup: f32) -> LineDrawable {
    // Calculate some points that look nice for an animated line.
    let lorenz_points = lorenz_points(seconds_since_startup);
    LineDrawable::new(
        re_ctx,
        &[
            // Complex orange line.
            LineStrip {
                points: lorenz_points.into(),
                radius: 0.05,
                color: [255, 191, 0, 255],
                flags: LineStripFlags::empty(),
            },
            // Green Zig-Zag
            LineStrip {
                points: smallvec![
                    glam::vec3(0.0, -1.0, 0.0),
                    glam::vec3(1.0, 0.0, 0.0),
                    glam::vec3(2.0, -1.0, 0.0),
                    glam::vec3(3.0, 0.0, 0.0),
                ],
                radius: 0.1,
                color: [50, 255, 50, 255],
                flags: LineStripFlags::CAP_END_TRIANGLE,
            },
            // A blue spiral
            LineStrip {
                points: (0..1000)
                    .map(|i| {
                        glam::vec3(
                            (i as f32 * 0.01).sin() * 2.0,
                            i as f32 * 0.01 - 6.0,
                            (i as f32 * 0.01).cos() * 2.0,
                        )
                    })
                    .collect(),
                radius: 0.1,
                color: [50, 50, 255, 255],
                flags: LineStripFlags::CAP_END_TRIANGLE,
            },
        ],
    )
    .unwrap()
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
                &mut re_ctx.mesh_manager,
                &mut re_ctx.texture_manager_2d,
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
        surface_configuration: &wgpu::SurfaceConfiguration,
        time: &framework::Time,
    ) -> Vec<(ViewBuilder, wgpu::CommandBuffer)> {
        if matches!(self.camera_control, CameraControl::RotateAroundCenter) {
            let seconds_since_startup = time.seconds_since_startup();
            self.camera_position = Vec3::new(
                seconds_since_startup.sin(),
                0.5,
                seconds_since_startup.cos(),
            ) * 10.0;
        }

        let resolution = [surface_configuration.width, surface_configuration.height];
        let seconds_since_startup = time.seconds_since_startup();
        let view_from_world =
            IsoTransform::look_at_rh(self.camera_position, Vec3::ZERO, Vec3::Y).unwrap();

        let triangle = TestTriangleDrawable::new(re_ctx);
        let skybox = GenericSkyboxDrawable::new(re_ctx);
        let lines = build_lines(re_ctx, seconds_since_startup);
        let point_cloud = PointCloudDrawable::new(re_ctx, &self.random_points).unwrap();
        let meshes = build_mesh_instances(
            re_ctx,
            &self.model_mesh_instances,
            &self.mesh_instance_positions_and_colors,
            seconds_since_startup,
        );

        let splits = split_resolution(resolution, 2, 2).collect::<Vec<_>>();

        let projection_from_view = if self.perspective_projection {
            Projection::Perspective {
                vertical_fov: 70.0 * TAU / 360.0,
                near_plane_distance: 0.01,
            }
        } else {
            Projection::Orthographic {
                vertical_world_size: 15.0,
                far_plane_distance: 100000.0,
            }
        };

        // Using a macro here because `Drawable` isn't object safe and a closure cannot be
        // generic over its input type.
        #[rustfmt::skip]
        macro_rules! draw {
            ($name:ident @ split #$n:expr) => {{
                let ((x, y), (width, height)) = splits[$n];
                draw_view(re_ctx,
                    TargetConfiguration {
                        name: stringify!($name).into(),
                        resolution_in_pixel: [width as u32, height as u32],
                        origin_in_pixel: [x as u32, y as u32],
                        view_from_world,
                        projection_from_view: projection_from_view.clone(),
                    },
                    &skybox,
                    &$name
                )
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
