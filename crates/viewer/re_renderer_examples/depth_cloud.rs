//! Demonstrates the use of our depth cloud renderer, which will efficiently draw a point cloud
//! using a depth texture and a set of intrinsics.
//!
//! ## Usage
//!
//! Native:
//! ```sh
//! cargo r -p re_renderer --example depth_cloud
//! ```
//!
//! Web:
//! ```sh
//! cargo run-wasm --example depth_cloud
//! ```

#![expect(clippy::disallowed_methods)] // allow hardcoded colors

use std::f32::consts::TAU;

use glam::Vec3;
use itertools::Itertools as _;
use macaw::IsoTransform;
use re_renderer::renderer::{
    ColormappedTexture, DepthCloud, DepthCloudDrawData, DepthClouds, DrawData,
    GenericSkyboxDrawData, RectangleDrawData, RectangleOptions, TexturedRect,
};
use re_renderer::resource_managers::{GpuTexture2D, ImageDataDesc};
use re_renderer::view_builder::{self, Projection, ViewBuilder};
use re_renderer::{Color32, LineDrawableBuilder, PointCloudBuilder, Rgba, Size};
use winit::event::ElementState;
use winit::keyboard;

mod framework;

// ---

// TODO(#1426): unify camera logic between examples.
enum CameraControl {
    RotateAroundCenter,

    // TODO(andreas): Only pauses rotation right now. Add camera controller.
    Manual,
}

struct RenderDepthClouds {
    depth: DepthTexture,
    albedo: AlbedoTexture,

    scale: f32,
    point_radius_from_world_depth: f32,
    intrinsics: glam::Mat3,

    camera_control: CameraControl,
    camera_position: glam::Vec3,
}

impl RenderDepthClouds {
    /// Manually backproject the depth texture into a point cloud and render it.
    fn draw_backprojected_point_cloud<FD, ID>(
        &mut self,
        re_ctx: &re_renderer::RenderContext,
        pixels_per_point: f32,
        resolution_in_pixel: [u32; 2],
        target_location: glam::Vec2,
        frame_draw_data: FD,
        image_draw_data: ID,
    ) -> anyhow::Result<framework::ViewDrawResult>
    where
        FD: DrawData + Sync + Send + Clone + 'static,
        ID: DrawData + Sync + Send + Clone + 'static,
    {
        let Self {
            depth,
            scale,
            point_radius_from_world_depth,
            intrinsics,
            ..
        } = self;

        let focal_length = glam::Vec2::new(intrinsics.x_axis.x, intrinsics.y_axis.y);
        let offset = glam::Vec2::new(intrinsics.z_axis.x, intrinsics.z_axis.y);

        let point_cloud_draw_data = {
            let (points, colors, radii): (Vec<_>, Vec<_>, Vec<_>) = (0..depth.dimensions.y)
                .flat_map(|y| (0..depth.dimensions.x).map(move |x| glam::UVec2::new(x, y)))
                .map(|texcoords| {
                    let linear_depth = depth.get_linear(texcoords.x, texcoords.y);
                    let pos_in_world = ((texcoords.as_vec2() - offset) * linear_depth
                        / focal_length)
                        .extend(linear_depth);

                    (
                        pos_in_world * *scale,
                        Color32::from_gray((linear_depth * 255.0) as u8),
                        Size(linear_depth * *point_radius_from_world_depth),
                    )
                })
                .multiunzip();

            let mut builder = PointCloudBuilder::new(re_ctx);
            builder
                .batch("backprojected point cloud")
                .add_points(&points, &radii, &colors, &[]);
            builder.into_draw_data()?
        };

        let mut view_builder = ViewBuilder::new(
            re_ctx,
            view_builder::TargetConfiguration {
                name: "Point Cloud".into(),
                resolution_in_pixel,
                view_from_world: IsoTransform::look_at_rh(
                    self.camera_position,
                    Vec3::ZERO,
                    Vec3::Y,
                )
                .ok_or_else(|| anyhow::format_err!("invalid camera"))?,
                projection_from_view: Projection::Perspective {
                    vertical_fov: 70.0 * std::f32::consts::TAU / 360.0,
                    near_plane_distance: 0.01,
                    aspect_ratio: resolution_in_pixel[0] as f32 / resolution_in_pixel[1] as f32,
                },
                pixels_per_point,
                ..Default::default()
            },
        )?;

        let command_buffer = view_builder
            .queue_draw(
                re_ctx,
                GenericSkyboxDrawData::new(re_ctx, Default::default()),
            )
            .queue_draw(re_ctx, point_cloud_draw_data)
            .queue_draw(re_ctx, frame_draw_data)
            .queue_draw(re_ctx, image_draw_data)
            .draw(re_ctx, re_renderer::Rgba::TRANSPARENT)?;

        Ok(framework::ViewDrawResult {
            view_builder,
            command_buffer,
            target_location,
        })
    }

    /// Pass the depth texture to our native depth cloud renderer.
    fn draw_depth_cloud<FD, ID>(
        &mut self,
        re_ctx: &re_renderer::RenderContext,
        pixels_per_point: f32,
        resolution_in_pixel: [u32; 2],
        target_location: glam::Vec2,
        frame_draw_data: FD,
        image_draw_data: ID,
    ) -> anyhow::Result<framework::ViewDrawResult>
    where
        FD: DrawData + Sync + Send + Clone + 'static,
        ID: DrawData + Sync + Send + Clone + 'static,
    {
        let Self {
            depth,
            scale,
            point_radius_from_world_depth,
            intrinsics,
            ..
        } = self;

        let world_from_rdf = glam::Affine3A::from_scale(glam::Vec3::splat(*scale));

        let depth_cloud_draw_data = DepthCloudDrawData::new(
            re_ctx,
            &DepthClouds {
                clouds: vec![DepthCloud {
                    world_from_rdf,
                    depth_camera_intrinsics: *intrinsics,
                    world_depth_from_texture_depth: 1.0,
                    point_radius_from_world_depth: *point_radius_from_world_depth,
                    min_max_depth_in_world: [0.0, 5.0],
                    depth_dimensions: depth.dimensions,
                    depth_texture: depth.texture.clone(),
                    colormap: re_renderer::Colormap::Turbo,
                    outline_mask_id: Default::default(),
                    picking_object_id: Default::default(),
                }],
                radius_boost_in_ui_points_for_outlines: 2.5,
            },
        )?;

        let mut view_builder = ViewBuilder::new(
            re_ctx,
            view_builder::TargetConfiguration {
                name: "Depth Cloud".into(),
                resolution_in_pixel,
                view_from_world: IsoTransform::look_at_rh(
                    self.camera_position,
                    Vec3::ZERO,
                    Vec3::Y,
                )
                .ok_or_else(|| anyhow::format_err!("invalid camera"))?,
                projection_from_view: Projection::Perspective {
                    vertical_fov: 70.0 * std::f32::consts::TAU / 360.0,
                    near_plane_distance: 0.01,
                    aspect_ratio: resolution_in_pixel[0] as f32 / resolution_in_pixel[1] as f32,
                },
                pixels_per_point,
                ..Default::default()
            },
        )?;

        let command_buffer = view_builder
            .queue_draw(
                re_ctx,
                GenericSkyboxDrawData::new(re_ctx, Default::default()),
            )
            .queue_draw(re_ctx, depth_cloud_draw_data)
            .queue_draw(re_ctx, frame_draw_data)
            .queue_draw(re_ctx, image_draw_data)
            .draw(re_ctx, re_renderer::Rgba::TRANSPARENT)?;

        Ok(framework::ViewDrawResult {
            view_builder,
            command_buffer,
            target_location,
        })
    }
}

impl framework::Example for RenderDepthClouds {
    fn title() -> &'static str {
        "Depth clouds"
    }

    fn new(re_ctx: &re_renderer::RenderContext) -> Self {
        re_log::info!("Stop camera movement by pressing 'Space'");

        let depth = DepthTexture::spiral(re_ctx, glam::uvec2(640, 480));
        let albedo = AlbedoTexture::spiral(re_ctx, depth.dimensions);

        let scale = 50.0;
        let point_radius_from_world_depth = 0.1;

        // hardcoded intrinsics for nyud dataset
        let focal_length = depth.dimensions.x as f32 * 0.7;
        let offset = depth.dimensions.as_vec2() * 0.5;
        let intrinsics = glam::Mat3::from_cols(
            Vec3::new(focal_length, 0.0, offset.x),
            Vec3::new(0.0, focal_length, offset.y),
            Vec3::new(0.0, 0.0, 1.0),
        )
        .transpose();

        Self {
            depth,
            albedo,

            scale,
            point_radius_from_world_depth,
            intrinsics,

            camera_control: CameraControl::RotateAroundCenter,
            camera_position: glam::Vec3::ZERO,
        }
    }

    fn draw(
        &mut self,
        re_ctx: &re_renderer::RenderContext,
        resolution: [u32; 2],
        time: &framework::Time,
        pixels_per_point: f32,
    ) -> anyhow::Result<Vec<framework::ViewDrawResult>> {
        let Self {
            albedo,
            camera_control,
            camera_position,
            ..
        } = self;

        let secs_since_startup = time.secs_since_startup();
        if matches!(camera_control, CameraControl::RotateAroundCenter) {
            *camera_position =
                Vec3::new(secs_since_startup.sin(), 0.5, secs_since_startup.cos()) * 100.0;
        }

        let splits = framework::split_resolution(resolution, 1, 2).collect::<Vec<_>>();

        let frame_size = albedo.dimensions.as_vec2().extend(0.0) / 15.0;
        let scale = glam::Affine3A::from_scale(frame_size);
        let rotation = glam::Affine3A::IDENTITY;
        let translation_center =
            glam::Affine3A::from_translation(-glam::Vec3::splat(0.5) * frame_size);
        let world_from_model = rotation * translation_center * scale;

        let frame_draw_data = {
            let mut builder = LineDrawableBuilder::new(re_ctx);
            {
                let mut line_batch = builder.batch("frame").world_from_obj(world_from_model);
                line_batch.add_box_outline_from_transform(
                    glam::Affine3A::from_scale_rotation_translation(
                        glam::Vec3::new(1.0, 1.0, 0.0),
                        Default::default(),
                        glam::Vec3::ONE * 0.5,
                    ),
                );
            }
            builder.into_draw_data()?
        };

        let image_draw_data = RectangleDrawData::new(
            re_ctx,
            &[TexturedRect {
                top_left_corner_position: world_from_model
                    .transform_point3(glam::Vec3::new(1.0, 1.0, 0.0)),
                extent_u: world_from_model.transform_vector3(-glam::Vec3::X),
                extent_v: world_from_model.transform_vector3(-glam::Vec3::Y),
                colormapped_texture: ColormappedTexture::from_unorm_rgba(albedo.texture.clone()),
                options: RectangleOptions {
                    texture_filter_magnification: re_renderer::renderer::TextureFilterMag::Nearest,
                    texture_filter_minification: re_renderer::renderer::TextureFilterMin::Linear,
                    multiplicative_tint: Rgba::from_white_alpha(0.5),
                    depth_offset: -1,
                    ..Default::default()
                },
            }],
        )?;

        Ok(vec![
            self.draw_backprojected_point_cloud(
                re_ctx,
                pixels_per_point,
                splits[0].resolution_in_pixel,
                splits[0].target_location,
                frame_draw_data.clone(),
                image_draw_data.clone(),
            )?,
            self.draw_depth_cloud(
                re_ctx,
                pixels_per_point,
                splits[1].resolution_in_pixel,
                splits[1].target_location,
                frame_draw_data,
                image_draw_data,
            )?,
        ])
    }

    fn on_key_event(&mut self, input: winit::event::KeyEvent) {
        #![expect(clippy::single_match)]
        match (input.state, input.logical_key) {
            (ElementState::Released, keyboard::Key::Named(keyboard::NamedKey::Space)) => {
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

/// Returns `(position_in_image, linear_depth)`.
fn spiral(dimensions: glam::UVec2) -> impl Iterator<Item = (glam::UVec2, f32)> {
    let size = (dimensions.x * dimensions.y) as usize;
    let factor = dimensions.as_vec2() - 1.0;

    let mut i = 0;
    let mut angle_rad: f32 = 0.0;

    std::iter::from_fn(move || {
        if i < size {
            let radius = i as f32 / size as f32;
            let pos = glam::Vec2::splat(0.5)
                + glam::Vec2::new(angle_rad.cos(), angle_rad.sin()) * 0.5 * radius;
            let texcoords = (pos * factor).as_uvec2();

            i += 1;
            angle_rad += 0.0005 * TAU;

            return Some((texcoords, radius));
        }

        None
    })
}

pub fn hash(value: &impl std::hash::Hash) -> u64 {
    ahash::RandomState::with_seeds(1, 2, 3, 4).hash_one(value)
}

struct DepthTexture {
    dimensions: glam::UVec2,
    data: Vec<f32>,
    texture: GpuTexture2D,
}

impl DepthTexture {
    pub fn spiral(re_ctx: &re_renderer::RenderContext, dimensions: glam::UVec2) -> Self {
        let size = (dimensions.x * dimensions.y) as usize;
        let mut data = vec![0f32; size];
        spiral(dimensions).for_each(|(texcoords, d)| {
            data[(texcoords.x + texcoords.y * dimensions.x) as usize] = d;
        });

        let label = format!("depth texture spiral {dimensions}");
        let texture = re_ctx
            .texture_manager_2d
            .get_or_create(
                hash(&label),
                re_ctx,
                ImageDataDesc {
                    label: label.into(),
                    data: bytemuck::cast_slice(&data).into(),
                    format: wgpu::TextureFormat::R32Float.into(),
                    width_height: dimensions.to_array(),
                    alpha_channel_usage: re_renderer::AlphaChannelUsage::Opaque,
                },
            )
            .expect("Failed to create depth texture.");

        Self {
            dimensions,
            data,
            texture,
        }
    }

    pub fn get_linear(&self, x: u32, y: u32) -> f32 {
        self.data[(x + y * self.dimensions.x) as usize]
    }
}

struct AlbedoTexture {
    dimensions: glam::UVec2,
    rgba8: Vec<u8>,
    texture: GpuTexture2D,
}

impl AlbedoTexture {
    pub fn spiral(re_ctx: &re_renderer::RenderContext, dimensions: glam::UVec2) -> Self {
        let size = (dimensions.x * dimensions.y) as usize;
        let mut rgba8 = vec![0; size * 4];
        spiral(dimensions).for_each(|(texcoords, d)| {
            let idx = ((texcoords.x + texcoords.y * dimensions.x) * 4) as usize;
            rgba8[idx..idx + 4].copy_from_slice(re_renderer::colormap_turbo_srgb(d).as_slice());
        });

        let label = format!("albedo texture spiral {dimensions}");
        let texture = re_ctx
            .texture_manager_2d
            .get_or_create(
                hash(&label),
                re_ctx,
                ImageDataDesc {
                    label: label.into(),
                    data: bytemuck::cast_slice(&rgba8).into(),
                    format: wgpu::TextureFormat::Rgba8UnormSrgb.into(),
                    width_height: dimensions.to_array(),
                    alpha_channel_usage: re_renderer::AlphaChannelUsage::Opaque,
                },
            )
            .expect("Failed to create albedo texture.");

        Self {
            dimensions,
            rgba8,
            texture,
        }
    }

    #[expect(dead_code)]
    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        let p = &self.rgba8[(x + y * self.dimensions.x) as usize * 4..];
        [p[0], p[1], p[2], p[3]]
    }
}
