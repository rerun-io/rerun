use std::path::PathBuf;

use ecolor::Hsva;
use glam::{UVec3, Vec2, Vec3, Vec3Swizzles};
use image::{DynamicImage, GenericImageView};
use itertools::Itertools;
use macaw::IsoTransform;
use rand::Rng;
use re_renderer::{
    renderer::{
        DepthCloud, DepthCloudDrawData, GenericSkyboxDrawData, LineStripFlags, RectangleDrawData,
        TextureFilterMag, TextureFilterMin, TexturedRect,
    },
    resource_managers::{GpuTexture2DHandle, Texture2DCreationDesc},
    view_builder::{self, Projection, TargetConfiguration, ViewBuilder},
    Color32, LineStripSeriesBuilder, PointCloudBuilder, Rgba, Size,
};
use winit::event::{ElementState, VirtualKeyCode};

mod framework;

enum CameraControl {
    RotateAroundCenter,

    // TODO(andreas): Only pauses rotation right now. Add camera controller.
    Manual,
}

#[derive(Debug, Clone, Copy)]
enum ProjectionKind {
    Orthographic,
    Perspective,
}

struct RenderDepthClouds {
    depth: DepthTexture,
    albedo: AlbedoTexture,
    albedo_handle: GpuTexture2DHandle,

    camera_control: CameraControl,
    camera_position: glam::Vec3,

    depth_kind: DepthKind,
    projection_kind: ProjectionKind,
}

#[derive(Debug, Clone, Copy)]
enum DepthKind {
    /// The depth represents the distance between the fragment and the camera plane.
    CameraPlane,
    /// The depth represents the distance between the fragment and the camera itself.
    CameraPosition,
}
struct DepthTexture {
    dimensions: glam::UVec2,
    // TODO: it doesn't matter how Z started, at this point we need it to be:
    // - linear
    // - 0.0 = near, 1.0 = far
    // - distance from camera
    d32: Vec<f32>,
}
impl DepthTexture {
    pub fn from_file(path: impl Into<PathBuf>, dimensions: Option<glam::UVec2>) -> Self {
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

        let path = path.into();

        let mut img = image::open(&path).unwrap();
        // TODO: to sample or not to sample is still very much the question
        if let Some(dimensions) = dimensions {
            img = img.resize(dimensions.x, dimensions.y, image::imageops::Triangle);
        }

        // TODO: is the depth texture..:
        // - linear?
        // - inversed?
        // - distance from camera plane or distance from camera?
        let (mut is_linear, mut n, mut f) = (true, 0.0, 0.0);
        let mut is_reversed = false;

        if path.to_string_lossy().contains("teardown") {
            (is_linear, n, f) = (false, 0.2, 500.0);
            is_reversed = false;
        }

        if path.to_string_lossy().contains("nyud") {
            (is_linear, n, f) = (true, 0.2, 500.0);
            is_reversed = false;
        }

        // TODO: how does one do that with an infinite plane tho?

        fn depth_to_view_depth(n: f32, f: f32, z: f32) -> f32 {
            n * f / (f - z * (f - n))
        }
        fn view_depth_to_capped_linear(n: f32, f: f32, vz: f32) -> f32 {
            let vz = f32::min(vz, f);
            (vz - n) / (f - n)
        }

        fn linearize_depth(n: f32, f: f32, z: f32) -> f32 {
            let vd = n * f / (f - z * (f - n));
            (vd - n) / (f - n)
        }

        let dimensions = glam::UVec2::new(img.width(), img.height());
        let data = img
            .pixels()
            .map(|(x, y, _)| {
                let mut d = get_norm_pixel(&img, x, y);

                if is_reversed {
                    d = 1.0 - d;
                }
                if !is_linear {
                    // d = linearize_depth(n, f, d);
                    d = depth_to_view_depth(n, f, d);
                    d = view_depth_to_capped_linear(n, f * 0.05, d);
                }

                d
            })
            .collect();

        Self {
            dimensions,
            d32: data,
        }
    }

    pub fn get(&self, x: u32, y: u32) -> f32 {
        self.d32[(x + y * self.dimensions.x) as usize]
    }
}

struct AlbedoTexture {
    dimensions: glam::UVec2,
    rgba8: Vec<u8>,
}
impl AlbedoTexture {
    pub fn from_file(path: impl Into<PathBuf>, dimensions: Option<glam::UVec2>) -> Self {
        let mut img = image::open(path.into()).unwrap();
        if let Some(dimensions) = dimensions {
            img = img.resize(dimensions.x, dimensions.y, image::imageops::Triangle);
        }

        let dimensions = glam::UVec2::new(img.width(), img.height());
        let rgba8 = img.pixels().flat_map(|(_, _, p)| p.0).collect();

        Self { dimensions, rgba8 }
    }

    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        let p = &self.rgba8[(x + y * self.dimensions.x) as usize * 4..];
        [p[0], p[1], p[2], p[3]]
    }
}

impl framework::Example for RenderDepthClouds {
    fn title() -> &'static str {
        "Depth clouds"
    }

    fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        re_log::info!("Stop camera movement by pressing 'Space'");
        re_log::info!("Change perspective by pressing 'P'");
        re_log::info!("Change depth interpreation by pressing 'D'");

        // let depth = DepthTexture::from_file("/tmp/teardown_depthfull.png", Some((640, 640).into()));
        // let albedo = AlbedoTexture::from_file("/tmp/teardown_albedo.png", depth.dimensions.into());
        let depth =
            DepthTexture::from_file("/tmp/nyud_depth.pgm", Some(glam::UVec2::new(640, 480)));
        let albedo = AlbedoTexture::from_file("/tmp/nyud_albedo.ppm", depth.dimensions.into());

        let albedo_handle = re_ctx.texture_manager_2d.create(
            &mut re_ctx.gpu_resources.textures,
            &Texture2DCreationDesc {
                label: "albedo".into(),
                data: bytemuck::cast_slice(&albedo.rgba8),
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                width: albedo.dimensions.x,
                height: albedo.dimensions.y,
            },
        );

        let depth_kind = DepthKind::CameraPlane;
        let projection_kind = ProjectionKind::Orthographic;

        re_log::info!(?depth_kind, ?projection_kind, "current settings");

        RenderDepthClouds {
            depth,
            albedo,
            albedo_handle,

            camera_control: CameraControl::RotateAroundCenter,
            camera_position: glam::Vec3::ZERO,

            depth_kind,
            projection_kind,
        }
    }

    fn draw(
        &mut self,
        re_ctx: &mut re_renderer::RenderContext,
        resolution: [u32; 2],
        time: &framework::Time,
        pixels_from_point: f32,
    ) -> Vec<framework::ViewDrawResult> {
        // volume_rgba8: Vec<u8>,
        // volume_size: Vec3,
        // volume_dimensions: UVec3,

        let Self {
            depth,
            albedo,
            albedo_handle,
            camera_control,
            camera_position,
            depth_kind,
            projection_kind,
        } = self;

        let seconds_since_startup = time.seconds_since_startup();
        if matches!(camera_control, CameraControl::RotateAroundCenter) {
            *camera_position = Vec3::new(
                seconds_since_startup.sin(),
                0.5,
                seconds_since_startup.cos(),
            ) * 100.0;
        }

        let splits = framework::split_resolution(resolution, 1, 1).collect::<Vec<_>>();

        let volume_size = albedo.dimensions.as_vec2().extend(0.0) / 10.0;
        let scale = glam::Mat4::from_scale(volume_size);
        let rotation = glam::Mat4::IDENTITY;
        let translation_center =
            glam::Mat4::from_translation(-glam::Vec3::splat(0.5) * volume_size);
        let translation_offset = glam::Mat4::from_translation(glam::Vec3::Z * 60.0);
        let world_from_model = rotation * translation_offset * translation_center * scale;
        let model_from_world = world_from_model.inverse();

        let mut bbox_builder = LineStripSeriesBuilder::<()>::default();
        {
            let mut line_batch = bbox_builder.batch("bbox").world_from_obj(world_from_model);
            line_batch.add_box_outline(glam::Affine3A::from_scale_rotation_translation(
                glam::Vec3::new(1.0, 1.0, 0.0),
                Default::default(),
                glam::Vec3::ONE * 0.5,
            ));
        }
        let bbox_draw_data = bbox_builder.to_draw_data(re_ctx);

        let rect_draw_data = RectangleDrawData::new(
            re_ctx,
            &[TexturedRect {
                top_left_corner_position: world_from_model
                    .transform_point3(glam::Vec3::new(0.0, 1.0, 0.0)),
                extent_u: world_from_model.transform_vector3(glam::Vec3::X),
                extent_v: world_from_model.transform_vector3(-glam::Vec3::Y),
                texture: albedo_handle.clone(),
                texture_filter_magnification: re_renderer::renderer::TextureFilterMag::Nearest,
                texture_filter_minification: re_renderer::renderer::TextureFilterMin::Linear,
                multiplicative_tint: Rgba::WHITE,
                // Push to background. Mostly important for mouse picking order!
                depth_offset: -1,
            }],
        )
        .unwrap();

        let depth_cloud_instances = vec![{
            DepthCloud {
                world_from_model,
                model_from_world,
                // dimensions: volume_dimensions,
                depth_dimensions: depth.dimensions,
                depth_data: depth.d32.clone(),
                // albedo_dimensions: albedo.dimensions,
                // albedo_data: albedo.rgba8.clone().into(),
            }
        }];

        let depth_cloud_draw_data =
            DepthCloudDrawData::new(re_ctx, &depth_cloud_instances).unwrap();

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
                    .queue_draw(&rect_draw_data)
                    .queue_draw(&depth_cloud_draw_data)
                    .queue_draw(&bbox_draw_data)
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
            (ElementState::Released, Some(VirtualKeyCode::Space)) => {
                self.camera_control = match self.camera_control {
                    CameraControl::RotateAroundCenter => CameraControl::Manual,
                    CameraControl::Manual => CameraControl::RotateAroundCenter,
                };
            }

            (ElementState::Released, Some(VirtualKeyCode::P)) => {
                self.projection_kind = match self.projection_kind {
                    ProjectionKind::Orthographic => ProjectionKind::Perspective,
                    ProjectionKind::Perspective => ProjectionKind::Orthographic,
                };
            }

            (ElementState::Released, Some(VirtualKeyCode::D)) => {
                self.depth_kind = match self.depth_kind {
                    DepthKind::CameraPlane => DepthKind::CameraPosition,
                    DepthKind::CameraPosition => DepthKind::CameraPlane,
                };
            }

            _ => {}
        }

        re_log::info!(depth_kind = ?self.depth_kind, projection_kind = ?self.projection_kind, "current settings");
    }
}

fn main() {
    framework::start::<RenderDepthClouds>();
}
