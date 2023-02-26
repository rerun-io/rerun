use std::path::PathBuf;

use anyhow::Context;
use ecolor::{linear_u8_from_linear_f32, Hsva};
use glam::{UVec3, Vec2, Vec3, Vec3Swizzles};
use image::{DynamicImage, GenericImageView};
use itertools::{multiunzip, Itertools};
use macaw::IsoTransform;
use rand::Rng;
use re_renderer::{
    renderer::{
        DepthCloud, DepthCloudDepthData, DepthCloudDrawData, GenericSkyboxDrawData, LineStripFlags,
        RectangleDrawData, TextureFilterMag, TextureFilterMin, TexturedRect,
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
struct RenderDepthClouds {
    depth: DepthTexture,
    albedo: AlbedoTexture,
    albedo_handle: GpuTexture2DHandle,

    camera_control: CameraControl,
    camera_position: glam::Vec3,
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
        let depth = DepthTexture::from_bytes(
            include_bytes!("assets/nyud_depth.pgm"),
            Some(glam::UVec2::new(640, 480)),
        );
        let albedo = AlbedoTexture::from_bytes(
            include_bytes!("assets/nyud_albedo.ppm"),
            depth.dimensions.into(),
        );

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

        RenderDepthClouds {
            depth,
            albedo,
            albedo_handle,

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
        let Self {
            depth,
            albedo,
            albedo_handle,
            camera_control,
            camera_position,
        } = self;

        let seconds_since_startup = time.seconds_since_startup();
        if matches!(camera_control, CameraControl::RotateAroundCenter) {
            *camera_position = Vec3::new(
                seconds_since_startup.sin(),
                0.5,
                seconds_since_startup.cos(),
            ) * 100.0;
        }

        let z_scale = 50.0;

        let focal_length = depth.dimensions.x as f32 * 0.7;
        let offset = depth.dimensions.as_vec2() * 0.5;
        let intrinsics = glam::Mat3::from_cols(
            Vec3::new(focal_length, 0.0, offset.x),
            Vec3::new(0.0, focal_length, offset.y),
            Vec3::new(0.0, 0.0, 1.0),
        );

        let splits = framework::split_resolution(resolution, 1, 2).collect::<Vec<_>>();

        let frame_size = albedo.dimensions.as_vec2().extend(0.0) / 10.0;
        let scale = glam::Mat4::from_scale(frame_size);
        let rotation = glam::Mat4::IDENTITY;
        let translation_center = glam::Mat4::from_translation(-glam::Vec3::splat(0.5) * frame_size);
        let world_from_model = rotation * translation_center * scale;

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
                    .transform_point3(glam::Vec3::new(1.0, 1.0, 0.0)),
                extent_u: world_from_model.transform_vector3(-glam::Vec3::X),
                extent_v: world_from_model.transform_vector3(-glam::Vec3::Y),
                texture: albedo_handle.clone(),
                texture_filter_magnification: re_renderer::renderer::TextureFilterMag::Nearest,
                texture_filter_minification: re_renderer::renderer::TextureFilterMin::Linear,
                multiplicative_tint: Rgba::from_white_alpha(0.5),
                depth_offset: -1,
            }],
        )
        .unwrap();

        let point_cloud_draw_data = {
            let num_points = depth.dimensions.x * depth.dimensions.y;
            let (points, colors, radii): (Vec<_>, Vec<_>, Vec<_>) = (0..depth.dimensions.y)
                .flat_map(|y| (0..depth.dimensions.x).map(move |x| glam::UVec2::new(x, y)))
                .map(|texcoords| {
                    let linear_depth = depth.get_linear(texcoords.x, texcoords.y);
                    let pos_in_world = ((texcoords.as_vec2() - offset) * linear_depth
                        / focal_length)
                        .extend(linear_depth);

                    (
                        pos_in_world * z_scale,
                        Color32::from_gray((linear_depth * 255.0) as u8),
                        Size(-linear_depth * 4.0),
                    )
                })
                .multiunzip();

            let mut builder = PointCloudBuilder::<()>::new(re_ctx);
            builder
                .batch("backprojected point cloud")
                .add_points(num_points as _, points.into_iter())
                .colors(colors.into_iter())
                .radii(radii.into_iter());

            builder.to_draw_data(re_ctx).unwrap()
        };

        let depth_cloud_draw_data = DepthCloudDrawData::new(
            re_ctx,
            &[DepthCloud {
                intrinsics,
                z_scale,
                depth_dimensions: depth.dimensions,
                depth_data: depth.data.clone(),
            }],
        )
        .unwrap();

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
                                Vec3::ZERO,
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
                    .queue_draw(&point_cloud_draw_data)
                    .queue_draw(&bbox_draw_data)
                    .queue_draw(&rect_draw_data)
                    .draw(re_ctx, ecolor::Rgba::TRANSPARENT)
                    .unwrap();
                framework::ViewDrawResult {
                    view_builder,
                    command_buffer,
                    target_location: splits[0].target_location,
                }
            }, //
            {
                let mut view_builder = ViewBuilder::default();
                view_builder
                    .setup_view(
                        re_ctx,
                        view_builder::TargetConfiguration {
                            name: "3D".into(),
                            resolution_in_pixel: splits[1].resolution_in_pixel,
                            view_from_world: IsoTransform::look_at_rh(
                                self.camera_position,
                                Vec3::ZERO,
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
                    .queue_draw(&depth_cloud_draw_data)
                    .queue_draw(&bbox_draw_data)
                    .queue_draw(&rect_draw_data)
                    .draw(re_ctx, ecolor::Rgba::TRANSPARENT)
                    .unwrap();
                framework::ViewDrawResult {
                    view_builder,
                    command_buffer,
                    target_location: splits[1].target_location,
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

            _ => {}
        }
    }
}

fn main() {
    framework::start::<RenderDepthClouds>();
}

// ---

struct DepthTexture {
    dimensions: glam::UVec2,
    data: DepthCloudDepthData,
}
impl DepthTexture {
    pub fn from_file(path: impl Into<PathBuf>, dimensions: Option<glam::UVec2>) -> Self {
        let path = path.into();

        let img = image::open(&path)
            .with_context(|| format!("{path:?}"))
            .unwrap();

        Self::from_bytes(img.as_bytes(), dimensions)
    }

    pub fn from_bytes(bytes: &[u8], dimensions: Option<glam::UVec2>) -> Self {
        let mut img = image::load_from_memory(bytes).unwrap();
        if let Some(dimensions) = dimensions {
            img = img.resize(dimensions.x, dimensions.y, image::imageops::Nearest);
        }

        let dimensions = glam::UVec2::new(img.width(), img.height());
        let data = match img {
            DynamicImage::ImageLuma16(img) => DepthCloudDepthData::U16(img.to_vec()),
            _ => unimplemented!(),
        };

        Self { dimensions, data }
    }

    pub fn get_linear(&self, x: u32, y: u32) -> f32 {
        match &self.data {
            DepthCloudDepthData::U16(data) => {
                data[(x + y * self.dimensions.x) as usize] as f32 / u16::MAX as f32
            }
            DepthCloudDepthData::F32(data) => data[(x + y * self.dimensions.x) as usize],
        }
    }
}

struct AlbedoTexture {
    dimensions: glam::UVec2,
    rgba8: Vec<u8>,
}
impl AlbedoTexture {
    pub fn from_file(path: impl Into<PathBuf>, dimensions: Option<glam::UVec2>) -> Self {
        let path = path.into();

        let img = image::open(&path)
            .with_context(|| format!("{path:?}"))
            .unwrap();

        Self::from_bytes(img.as_bytes(), dimensions)
    }

    pub fn from_bytes(bytes: &[u8], dimensions: Option<glam::UVec2>) -> Self {
        let mut img = image::load_from_memory(bytes).unwrap();

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
