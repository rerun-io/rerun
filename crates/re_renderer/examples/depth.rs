use ecolor::Hsva;
use glam::{UVec3, Vec2, Vec3, Vec3Swizzles};
use image::{DynamicImage, GenericImageView};
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

    camera_control: CameraControl,
    camera_position: glam::Vec3,
}

impl framework::Example for RenderVolumetric {
    fn title() -> &'static str {
        "Volumetric Rendering"
    }

    fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        re_log::info!("Stop camera movement by pressing 'Space'");

        let img = image::open("/tmp/teardown_depth.png").unwrap();
        dbg!(img.width(), img.height());
        let img = img.resize_exact(640, 480, image::imageops::Nearest);
        let albedo = image::open("/tmp/teardown_albedo.png").unwrap();
        let albedo = albedo.resize_exact(640, 480, image::imageops::Nearest);

        // let img = image::open("/tmp/nyud_depth.pgm").unwrap();
        // dbg!(img.width(), img.height());
        // // let img = img.resize_exact(640, 480, image::imageops::Nearest);
        // // TODO: does the albedo texture need any flipping on X and/or Y?
        // let albedo = image::open("/tmp/nyud_albedo.ppm").unwrap();
        // // let albedo = albedo.resize_exact(640, 480, image::imageops::Nearest);

        fn get_norm_pixel(img: &DynamicImage, x: u32, y: u32) -> f32 {
            match &img {
                DynamicImage::ImageLuma8(img) => img.get_pixel(x, y).0[0] as f32 / u8::MAX as f32,
                DynamicImage::ImageLumaA8(_) => todo!(),
                DynamicImage::ImageRgb8(img) => img.get_pixel(x, y).0[0] as f32 / u8::MAX as f32,
                DynamicImage::ImageRgba8(img) => img.get_pixel(x, y).0[0] as f32 / u8::MAX as f32,
                DynamicImage::ImageLuma16(img) => img.get_pixel(x, y).0[0] as f32 / u16::MAX as f32,
                DynamicImage::ImageLumaA16(_) => todo!(),
                DynamicImage::ImageRgb16(_) => todo!(),
                DynamicImage::ImageRgba16(_) => todo!(),
                DynamicImage::ImageRgb32F(_) => todo!(),
                DynamicImage::ImageRgba32F(_) => todo!(),
                _ => todo!(),
            }
        }

        let img_size = Vec2::new(img.width() as f32, img.height() as f32);

        // TODO: Z is arbitrary I guess?
        let vol_size = Vec3::new(img.width() as f32, img.height() as f32, 640.0 * 1.0) * 0.15;
        // TODO: shouldnt have to be cubic
        let vol_dimensions = UVec3::new(640, 640, 640) / 4;

        dbg!(img_size);
        dbg!(vol_size);
        dbg!(vol_dimensions);

        let mut faked =
            vec![0u8; (vol_dimensions.x * vol_dimensions.y * vol_dimensions.z * 4) as usize];

        // TODO: somehow this needs to happen in a pre-pass then?
        let mut pixels_set = 0;
        let (mut zmin, mut zmax) = (f32::MAX, f32::MIN);
        for (x, y, _) in img.pixels() {
            // TODO: is the depth texture..:
            // - linear?
            // - inversed?
            // - distance from camera plane or distance from camera?
            let z = get_norm_pixel(&img, x, y);
            zmin = f32::min(zmin, z);
            zmax = f32::max(zmax, z);

            // TODO: it doesn't matter how Z started, at this point we need it to be:
            // - linear
            // - 0.0 = near, 1.0 = far
            // - distance from camera

            // Compute texture coordinates in the depth image's space.
            let texcoords = Vec2::new(x as f32, y as f32) / img_size;

            // Compute texture coordinates in the volume's back panel space (z=1.0).
            // let texcoords_in_volume = texcoords.extend(1.0);
            let texcoords_in_volume = Vec3::new(1.0 - texcoords.x, 1.0 - texcoords.y, 1.0);

            // Assume a virtual camera sitting at the center of the volume's front panel (z=0.0).
            let cam_npos_in_volume = Vec3::new(0.5, 0.5, 0.0);

            let npos_in_volume =
                cam_npos_in_volume + (texcoords_in_volume - cam_npos_in_volume) * z;
            let pos_in_volume = npos_in_volume * (vol_dimensions.as_vec3() - 1.0);

            let pos = pos_in_volume.as_uvec3();

            let idx = (pos.x
                + pos.y * vol_dimensions.x
                + pos.z * vol_dimensions.x * vol_dimensions.y) as usize;
            let idx = idx * 4;

            faked[idx..idx + 4].copy_from_slice(&albedo.get_pixel(x, y).0);
            pixels_set += 1;
        }

        dbg!((zmin, zmax));
        dbg!(pixels_set);

        RenderVolumetric {
            checkerboard: faked,
            checkerboard_size: vol_size,
            checkerboard_dimensions: vol_dimensions,

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
        // let seconds_since_startup = 0f32;
        let seconds_since_startup = time.seconds_since_startup();
        if matches!(self.camera_control, CameraControl::RotateAroundCenter) {
            self.camera_position = Vec3::new(
                seconds_since_startup.sin(),
                0.5,
                seconds_since_startup.cos(),
            ) * 100.0;
        }

        let splits = framework::split_resolution(resolution, 1, 1).collect::<Vec<_>>();

        let mut bbox_builder = LineStripSeriesBuilder::<()>::default();
        let volume_instances = vec![{
            let scale = glam::Mat4::from_scale(self.checkerboard_size);

            let rotation = glam::Mat4::IDENTITY;

            let translation_center =
                glam::Mat4::from_translation(-glam::Vec3::splat(0.5) * self.checkerboard_size);

            let world_from_model = rotation * translation_center * scale;
            let model_from_world = world_from_model.inverse();

            let mut line_batch = bbox_builder.batch("bbox").world_from_obj(world_from_model);
            line_batch.add_box_outline(glam::Affine3A::from_scale_rotation_translation(
                glam::Vec3::ONE,
                Default::default(),
                glam::Vec3::ONE * 0.5,
            ));

            Volume {
                world_from_model,
                model_from_world,
                size: self.checkerboard_size,
                dimensions: self.checkerboard_dimensions,
                data: self.checkerboard.clone(),
            }
        }];

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
                    .queue_draw(&bbox_builder.to_draw_data(re_ctx))
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
