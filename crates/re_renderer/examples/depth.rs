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
        TextureFilterMin, TexturedRect, Volume2D, Volume2DDrawData, Volume3D, Volume3DDrawData,
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

#[derive(Debug, Clone, Copy)]
enum ProjectionKind {
    Orthographic,
    Perspective,
}

struct RenderVolumetric {
    depth: DepthTexture,
    albedo: AlbedoTexture,

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

impl framework::Example for RenderVolumetric {
    fn title() -> &'static str {
        "Volumetric Rendering"
    }

    fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        re_log::info!("Stop camera movement by pressing 'Space'");
        re_log::info!("Change perspective by pressing 'P'");
        re_log::info!("Change depth interpreation by pressing 'D'");

        // let depth = DepthTexture::from_file("/tmp/teardown_depthfull.png", Some((640, 640).into()));
        // let albedo = AlbedoTexture::from_file("/tmp/teardown_albedo.png", depth.dimensions.into());
        let depth = DepthTexture::from_file("/tmp/nyud_depth.pgm", None);
        let albedo = AlbedoTexture::from_file("/tmp/nyud_albedo.ppm", depth.dimensions.into());

        let depth_kind = DepthKind::CameraPlane;
        let projection_kind = ProjectionKind::Orthographic;

        re_log::info!(?depth_kind, ?projection_kind, "current settings");

        RenderVolumetric {
            depth,
            albedo,

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
            camera_control,
            camera_position,
            depth_kind,
            projection_kind,
        } = self;

        let depth_size = depth.dimensions.as_vec2();

        // TODO: we can pretend that the current texture is the image plane, thus we need to
        // compute the dimensions of the far plane such that

        // TODO: the only thing that should ever change, then, is the size

        // TODO: Z is arbitrary I guess?
        // TODO: what about the volume size in world space? is this actually arbitrary? I guess it
        //       can be computed in a way that makes sense, somehow..?
        let volume_size = depth_size.extend(650.0) * 0.2;
        // TODO: shouldnt have to be cubic
        let volume_dimensions = UVec3::new(640, 640, 640) / 4;
        // let vol_dimensions =
        //     UVec3::new(img.width(), img.height(), (img.width() as f32 * 0.7) as u32) / 4;

        let mut volume3d_rgba8 = vec![
            0u8;
            (volume_dimensions.x * volume_dimensions.y * volume_dimensions.z * 4)
                as usize
        ];

        for (x, y) in
            (0..depth.dimensions.y).flat_map(|y| (0..depth.dimensions.x).map(move |x| (x, y)))
        {
            let z = depth.get(x, y); // linear, near=0.0

            // Compute texture coordinates in the depth image's space.
            let texcoords = Vec2::new(x as f32, y as f32) / depth_size;

            // Compute texture coordinates in the volume's back panel space (z=1.0).
            // let texcoords_in_volume = texcoords.extend(1.0);
            let texcoords_in_volume = Vec3::new(texcoords.x, 1.0 - texcoords.y, 0.0);

            let cam_npos_in_volume = match *projection_kind {
                ProjectionKind::Orthographic => texcoords_in_volume.xy().extend(1.0),
                ProjectionKind::Perspective => Vec3::new(0.5, 0.5, 1.0),
            };

            let z = match (*projection_kind, *depth_kind) {
                (ProjectionKind::Orthographic, DepthKind::CameraPlane) => z,
                (ProjectionKind::Orthographic, DepthKind::CameraPosition) => {
                    // TODO: compute planar-based
                    z
                }
                (ProjectionKind::Perspective, DepthKind::CameraPlane) => {
                    // TODO: compute position-based
                    z
                }
                (ProjectionKind::Perspective, DepthKind::CameraPosition) => z,
            };

            let npos_in_volume =
                cam_npos_in_volume + (texcoords_in_volume - cam_npos_in_volume) * z;
            let pos_in_volume = npos_in_volume * (volume_dimensions.as_vec3() - 1.0);

            let pos = pos_in_volume.as_uvec3();

            let idx = (pos.x
                + pos.y * volume_dimensions.x
                + pos.z * volume_dimensions.x * volume_dimensions.y) as usize;
            let idx = idx * 4;

            volume3d_rgba8[idx..idx + 4].copy_from_slice(&albedo.get(x, y));

            // let d = (z * 255.0) as u8;
            // faked[idx..idx + 4].copy_from_slice(&[d, d, d, 255]);
        }

        // let seconds_since_startup = 0f32;
        let seconds_since_startup = time.seconds_since_startup();
        if matches!(camera_control, CameraControl::RotateAroundCenter) {
            *camera_position = Vec3::new(
                seconds_since_startup.sin(),
                0.5,
                seconds_since_startup.cos(),
            ) * volume_size.max_element();
        }

        let splits = framework::split_resolution(resolution, 1, 2).collect::<Vec<_>>();

        let scale = glam::Mat4::from_scale(volume_size);
        let rotation = glam::Mat4::IDENTITY;
        let translation_center =
            glam::Mat4::from_translation(-glam::Vec3::splat(0.5) * volume_size);
        let world_from_model = rotation * translation_center * scale;
        let model_from_world = world_from_model.inverse();

        let mut bbox_builder = LineStripSeriesBuilder::<()>::default();
        {
            let mut line_batch = bbox_builder.batch("bbox").world_from_obj(world_from_model);
            line_batch.add_box_outline(glam::Affine3A::from_scale_rotation_translation(
                glam::Vec3::ONE,
                Default::default(),
                glam::Vec3::ONE * 0.5,
            ));
        }
        let bbox_draw_data = bbox_builder.to_draw_data(re_ctx);

        let volume3d_instances = vec![{
            Volume3D {
                world_from_model,
                model_from_world,
                size: volume_size,
                dimensions: volume_dimensions,
                data: volume3d_rgba8,
            }
        }];
        let volume2d_instances = vec![{
            Volume2D {
                world_from_model,
                model_from_world,
                dimensions: volume_dimensions,
                depth_dimensions: depth.dimensions,
                depth_data: depth.d32.clone(),
                albedo_dimensions: albedo.dimensions,
                albedo_data: albedo.rgba8.clone().into(),
            }
        }];

        let volume3d_draw_data = Volume3DDrawData::new(re_ctx, &volume3d_instances).unwrap();
        let volume2d_draw_data = Volume2DDrawData::new(re_ctx, &volume2d_instances).unwrap();

        vec![
            {
                let mut view_builder = ViewBuilder::default();
                view_builder
                    .setup_view(
                        re_ctx,
                        view_builder::TargetConfiguration {
                            name: "Volume 3D".into(),
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
                    .queue_draw(&volume3d_draw_data)
                    .queue_draw(&bbox_draw_data)
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
                    .queue_draw(&volume2d_draw_data)
                    .queue_draw(&bbox_draw_data)
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
    framework::start::<RenderVolumetric>();
}
