use ecolor::Hsva;
use glam::{UVec3, Vec3};
use itertools::Itertools;
use macaw::IsoTransform;
use rand::Rng;
use re_renderer::{
    renderer::{
        GenericSkyboxDrawData, LineStripFlags, RectangleDrawData, TextureFilterMag,
        TextureFilterMin, TexturedRect, Volume, VolumeDrawData,
    },
    resource_managers::{GpuTexture2DHandle, Texture2DCreationDesc},
    view_builder::{self, Projection, TargetConfiguration, ViewBuilder},
    Color32, LineStripSeriesBuilder, PointCloudBuilder, Size,
};
use winit::event::{ElementState, VirtualKeyCode};

mod framework;

enum CameraControl {
    RotateAroundCenter,

    // TODO(andreas): Only pauses rotation right now. Add camera controller.
    Manual,
}

struct RenderVolumetric {
    checkerboard: Vec<u8>,
    checkerboard_size: Vec3,
    checkerboard_dimensions: UVec3,

    initial_rotations: Vec<glam::Quat>,

    camera_control: CameraControl,
    camera_position: glam::Vec3,
}

impl framework::Example for RenderVolumetric {
    fn title() -> &'static str {
        "Volumetric Rendering"
    }

    fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        re_log::info!("Stop camera movement by pressing 'Space'");

        const R: [u8; 4] = [255, 0, 0, 255];
        const Y: [u8; 4] = [255, 255, 0, 255];
        const G: [u8; 4] = [0, 255, 0, 255];
        const EMPTY: [u8; 4] = [0, 0, 0, 0];

        const DIMENSION: u32 = 32;

        let size = Vec3::splat(DIMENSION as f32); // TODO
        let dimensions = UVec3::splat(DIMENSION);

        let mut rng = rand::thread_rng();
        let mut pos = UVec3::ZERO;
        let checkerboard = std::iter::repeat_with(|| {
            let lo = UVec3::splat(DIMENSION / 2 - DIMENSION / 10);
            let hi = UVec3::splat(DIMENSION / 2 + DIMENSION / 10);

            let mlo = UVec3::splat(DIMENSION / 2 - DIMENSION / 5);
            let mhi = UVec3::splat(DIMENSION / 2 + DIMENSION / 5);

            let color = if pos.x >= lo.x
                && pos.x <= hi.x
                && pos.y >= lo.y
                && pos.y <= hi.y
                && pos.z >= lo.z
                && pos.z <= hi.z
            {
                if rng.gen_ratio(30, 100) {
                    R
                } else if rng.gen_ratio(4, 1000) {
                    Y
                } else if rng.gen_ratio(4, 10000) {
                    G
                } else {
                    EMPTY
                }
            } else if pos.x >= mlo.x
                && pos.x <= mhi.x
                && pos.y >= mlo.y
                && pos.y <= mhi.y
                && pos.z >= mlo.z
                && pos.z <= mhi.z
            {
                if rng.gen_ratio(15, 100) {
                    Y
                } else if rng.gen_ratio(4, 1000) {
                    R
                } else if rng.gen_ratio(4, 10000) {
                    G
                } else {
                    EMPTY
                }
            } else {
                if rng.gen_ratio(4, 100) {
                    G
                } else if rng.gen_ratio(4, 1000) {
                    Y
                } else if rng.gen_ratio(4, 10000) {
                    R
                } else {
                    EMPTY
                }
            };

            pos += UVec3::new(1, 0, 0);
            if pos.x >= DIMENSION {
                pos.x = 0;
                pos.y += 1;
                if pos.y >= DIMENSION {
                    pos.y = 0;
                    pos.z += 1;
                }
            }

            color
        })
        .take((DIMENSION * DIMENSION * DIMENSION) as _)
        .flatten()
        .collect::<Vec<_>>();
        // let checkerboard = std::iter::repeat([R, G, B])
        //     .take((DIMENSION * DIMENSION * DIMENSION) as _)
        //     .flatten()
        //     .flatten()
        //     .collect::<Vec<_>>();

        let initial_rotations = (0..1000)
            .map(|_| {
                glam::Quat::from_vec4(glam::vec4(rng.gen(), rng.gen(), rng.gen(), 0.0).normalize())
            })
            .collect_vec();

        RenderVolumetric {
            checkerboard,
            checkerboard_size: size,
            checkerboard_dimensions: dimensions,

            initial_rotations,

            camera_control: CameraControl::RotateAroundCenter,
            camera_position: glam::Vec3::ZERO,
        }
    }

    fn draw(
        &mut self,
        re_ctx: &mut re_renderer::RenderContext,
        resolution: [u32; 2],
        time: &framework::Time,
        pixels_from_point: f32,
    ) -> Vec<framework::ViewDrawResult> {
        let seconds_since_startup = time.seconds_since_startup();
        if matches!(self.camera_control, CameraControl::RotateAroundCenter) {
            self.camera_position = Vec3::new(
                seconds_since_startup.sin(),
                0.5,
                seconds_since_startup.cos(),
            ) * 80.0;
        }

        let splits = framework::split_resolution(resolution, 1, 1).collect::<Vec<_>>();

        let volume_instances = (0..10)
            .flat_map(move |x| (0..10).map(move |z| (x, z)))
            .map(|(x, z)| {
                let idx = x + z * 10;
                let x = (x as f32 - 5.0) * self.checkerboard_size.x * 2.0;
                let z = (z as f32 - 5.0) * self.checkerboard_size.z * 2.0;

                let scale = glam::Mat4::from_scale(self.checkerboard_size);

                let rotation = self.initial_rotations[idx]
                    * glam::Quat::from_rotation_y(seconds_since_startup * 2.0);
                let rotation = glam::Mat4::from_quat(rotation);

                let translation_center =
                    glam::Mat4::from_translation(-glam::Vec3::splat(0.5) * self.checkerboard_size);
                let translation = glam::Mat4::from_translation(glam::Vec3::new(x, 0.0, z));

                let world_from_model = translation * rotation * translation_center * scale;
                let model_from_world = world_from_model.inverse();

                Volume {
                    world_from_model,
                    model_from_world,
                    size: self.checkerboard_size,
                    dimensions: self.checkerboard_dimensions,
                    data: self.checkerboard.clone(),
                }
            })
            .collect_vec();

        // let world_from_model = glam::Mat4::from_scale_rotation_translation(
        //     self.checkerboard_size,
        //     glam::Quat::from_rotation_z(seconds_since_startup),
        //     -glam::Vec3::splat(0.5) * self.checkerboard_size,
        // );

        let volume_draw_data = VolumeDrawData::new(re_ctx, &volume_instances).unwrap();

        vec![
            {
                let mut view_builder = ViewBuilder::default();
                view_builder
                    .setup_view(
                        re_ctx,
                        view_builder::TargetConfiguration {
                            name: "3D".into(),
                            resolution_in_pixel: splits[0].resolution_in_pixel,
                            view_from_world: IsoTransform::look_at_rh(
                                self.camera_position,
                                Vec3::ZERO, // TODO
                                Vec3::Y,
                            )
                            .unwrap(),
                            projection_from_view: Projection::Perspective {
                                vertical_fov: 70.0 * std::f32::consts::TAU / 360.0,
                                near_plane_distance: 0.01,
                            },
                            pixels_from_point,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                let command_buffer = view_builder
                    .queue_draw(&GenericSkyboxDrawData::new(re_ctx))
                    .queue_draw(&volume_draw_data)
                    .draw(re_ctx, ecolor::Rgba::TRANSPARENT)
                    .unwrap();
                framework::ViewDrawResult {
                    view_builder,
                    command_buffer,
                    target_location: splits[0].target_location,
                }
            }, //
        ]
    }

    fn on_keyboard_input(&mut self, input: winit::event::KeyboardInput) {
        match (input.state, input.virtual_keycode) {
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
    framework::start::<RenderVolumetric>();
}
