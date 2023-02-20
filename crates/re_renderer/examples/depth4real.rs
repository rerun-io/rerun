use std::path::PathBuf;

use ecolor::Hsva;
use glam::{UVec3, Vec2, Vec3, Vec3Swizzles};
use image::{DynamicImage, GenericImageView};
use itertools::Itertools;
use macaw::IsoTransform;
use rand::Rng;
use re_renderer::{
    renderer::{
        GenericSkyboxDrawData, LineStripFlags, RectangleDrawData, TextureFilterMag,
        TextureFilterMin, TexturedRect, Volume2D as Volume, Volume2DDrawData as VolumeDrawData,
    },
    resource_managers::{GpuTexture2DHandle, Texture2DCreationDesc},
    view_builder::{self, Projection, TargetConfiguration, ViewBuilder},
    Color32, LineStripSeriesBuilder, PointCloudBuilder, Size,
};
use winit::event::{ElementState, VirtualKeyCode};

#[path = "./framework.rs"]
mod framework;

enum CameraControl {
    RotateAroundCenter,

    // TODO(andreas): Only pauses rotation right now. Add camera controller.
    Manual,
}

struct RenderVolumetric {
    volume_size: glam::Vec3,
    volume_dimensions: glam::UVec3,

    depth: Vec<f32>,
    depth_dimensions: glam::UVec2,

    albedo: Vec<u8>,
    albedo_dimensions: glam::UVec2,

    camera_control: CameraControl,
    camera_position: glam::Vec3,
}

// TODO: it doesn't matter how Z started, at this point we need it to be:
// - linear
// - 0.0 = near, 1.0 = far
// - distance from camera
fn load_normalized_depth(
    path: impl Into<PathBuf>,

    dimensions: Option<glam::UVec2>,
) -> (glam::UVec2, Vec<f32>) {
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

    let mut img = image::open(path.into()).unwrap();
    if let Some(dimensions) = dimensions {
        img = img.resize(dimensions.x, dimensions.y, image::imageops::Nearest);
    }

    let dimensions = glam::UVec2::new(img.width(), img.height());
    let data = img
        .pixels()
        // .map(|(x, y, _)| [(get_norm_pixel(&img, x, y) * u16::MAX as f32) as u16; 1])
        // .flatten()
        .map(|(x, y, _)| get_norm_pixel(&img, x, y))
        .collect();

    (dimensions, data)
}

// NOTE: it converts too!
fn load_albedo(
    path: impl Into<PathBuf>,
    dimensions: Option<glam::UVec2>,
) -> (glam::UVec2, Vec<u8>) {
    let mut img = image::open(path.into()).unwrap();
    if let Some(dimensions) = dimensions {
        img = img.resize(dimensions.x, dimensions.y, image::imageops::Nearest);
    }

    let dimensions = glam::UVec2::new(img.width(), img.height());
    let data = img.pixels().flat_map(|(_, _, p)| p.0).collect();

    (dimensions, data)
}

impl framework::Example for RenderVolumetric {
    fn title() -> &'static str {
        "Volumetric Rendering"
    }

    fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        re_log::info!("Stop camera movement by pressing 'Space'");

        let (depth_dimensions, depth) =
            load_normalized_depth("/tmp/teardown_depth2.png", Some((640, 640).into()));
        // let (depth_dimensions, depth) = load_normalized_depth("/tmp/nyud_depth.pgm", None);
        let (albedo_dimensions, albedo) =
            load_albedo("/tmp/teardown_albedo.png", Some((640, 640).into()));
        // let (albedo_dimensions, albedo) = load_albedo("/tmp/nyud_albedo.ppm", None);

        // TODO: exactly what does `depth_dimensions != albedo_dimensions` implies?

        // TODO: Z is arbitrary I guess?
        let volume_size = Vec3::new(
            depth_dimensions.x as f32,
            depth_dimensions.y as f32,
            depth_dimensions.x as f32 * 0.7, // TODO
        ) * 0.15; // TODO

        // TODO: shouldnt have to be cubic to work
        let volume_dimensions = UVec3::new(640, 640, 640) / 4;

        RenderVolumetric {
            volume_size,
            volume_dimensions,

            depth,
            depth_dimensions,

            albedo,
            albedo_dimensions,

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
        // let seconds_since_startup = 3f32;
        if matches!(self.camera_control, CameraControl::RotateAroundCenter) {
            self.camera_position = Vec3::new(
                seconds_since_startup.sin(),
                0.5,
                seconds_since_startup.cos(),
            ) * 100.0;
        }

        // self.camera_position = Vec3::new(25.0, 50.0, 100.0);

        let splits = framework::split_resolution(resolution, 1, 1).collect::<Vec<_>>();

        let mut bbox_builder = LineStripSeriesBuilder::<()>::default();
        let volume_instances = vec![{
            let scale = glam::Mat4::from_scale(self.volume_size);

            let rotation = glam::Mat4::IDENTITY;

            let translation_center =
                glam::Mat4::from_translation(-glam::Vec3::splat(0.5) * self.volume_size);

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
                dimensions: self.volume_dimensions,
                depth_dimensions: self.depth_dimensions,
                depth_data: self.depth.clone(),
                albedo_dimensions: self.albedo_dimensions,
                albedo_data: self.albedo.clone(),
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
