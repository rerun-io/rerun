use ecolor::Hsva;
use re_renderer::{
    renderer::{
        ColormappedTexture, LineStripFlags, RectangleDrawData, RectangleOptions, TextureFilterMag,
        TextureFilterMin, TexturedRect,
    },
    resource_managers::{GpuTexture2D, Texture2DCreationDesc},
    view_builder::{self, Projection, TargetConfiguration, ViewBuilder},
    Color32, LineStripSeriesBuilder, PointCloudBuilder, Size,
};

mod framework;

struct Render2D {
    rerun_logo_texture: GpuTexture2D,
    rerun_logo_texture_width: u32,
    rerun_logo_texture_height: u32,
}

impl framework::Example for Render2D {
    fn title() -> &'static str {
        "2D Rendering"
    }

    fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        let rerun_logo =
            image::load_from_memory(include_bytes!("../../re_ui/data/logo_dark_mode.png")).unwrap();

        let image_data = rerun_logo.as_rgba8().unwrap().to_vec();

        let rerun_logo_texture = re_ctx
            .texture_manager_2d
            .create(
                &mut re_ctx.gpu_resources.textures,
                &Texture2DCreationDesc {
                    label: "rerun logo".into(),
                    data: image_data.into(),
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    width: rerun_logo.width(),
                    height: rerun_logo.height(),
                },
            )
            .expect("Failed to create texture for rerun logo");
        Render2D {
            rerun_logo_texture,

            rerun_logo_texture_width: rerun_logo.width(),
            rerun_logo_texture_height: rerun_logo.height(),
        }
    }

    fn draw(
        &mut self,
        re_ctx: &mut re_renderer::RenderContext,
        resolution: [u32; 2],
        time: &framework::Time,
        pixels_from_point: f32,
    ) -> Vec<framework::ViewDrawResult> {
        let splits = framework::split_resolution(resolution, 1, 2).collect::<Vec<_>>();

        let screen_size = glam::vec2(
            splits[0].resolution_in_pixel[0] as f32,
            splits[0].resolution_in_pixel[1] as f32,
        );

        let mut line_strip_builder = LineStripSeriesBuilder::new(re_ctx);

        // Blue rect outline around the bottom right quarter.
        {
            let mut line_batch = line_strip_builder.batch("quads");
            let line_radius = 10.0;
            let blue_rect_position = screen_size * 0.5 - glam::vec2(line_radius, line_radius);
            line_batch
                .add_rectangle_outline_2d(
                    blue_rect_position,
                    glam::vec2(screen_size.x * 0.5, 0.0),
                    glam::vec2(0.0, screen_size.y * 0.5),
                )
                .radius(Size::new_scene(line_radius))
                .color(Color32::BLUE);

            // .. within, a orange rectangle
            line_batch
                .add_rectangle_outline_2d(
                    blue_rect_position + screen_size * 0.125,
                    glam::vec2(screen_size.x * 0.25, 0.0),
                    glam::vec2(0.0, screen_size.y * 0.25),
                )
                .radius(Size::new_scene(5.0))
                .color(Color32::from_rgb(255, 100, 1));
        }

        // All variations of line caps
        {
            let mut line_batch = line_strip_builder.batch("line cap variations");
            for (i, flags) in [
                LineStripFlags::empty(),
                LineStripFlags::FLAG_CAP_START_ROUND,
                LineStripFlags::FLAG_CAP_END_ROUND,
                LineStripFlags::FLAG_CAP_START_TRIANGLE,
                LineStripFlags::FLAG_CAP_END_TRIANGLE,
                LineStripFlags::FLAG_CAP_START_ROUND | LineStripFlags::FLAG_CAP_END_ROUND,
                LineStripFlags::FLAG_CAP_START_ROUND | LineStripFlags::FLAG_CAP_END_TRIANGLE,
                LineStripFlags::FLAG_CAP_START_TRIANGLE | LineStripFlags::FLAG_CAP_END_ROUND,
                LineStripFlags::FLAG_CAP_START_TRIANGLE | LineStripFlags::FLAG_CAP_END_TRIANGLE,
            ]
            .iter()
            .enumerate()
            {
                let y = (i + 1) as f32 * 70.0;
                line_batch
                    .add_segment_2d(glam::vec2(70.0, y), glam::vec2(400.0, y))
                    .radius(Size::new_scene(15.0))
                    .flags(*flags | LineStripFlags::FLAG_COLOR_GRADIENT);
            }
        }

        // Lines with different kinds of radius
        // The first two lines are the same thickness if there no (!) scaling.
        // Moving the windows to a high dpi screen makes the second one bigger.
        // Also, it looks different under perspective projection.
        // The third line is automatic thickness which is determined by the line renderer implementation.
        {
            let mut line_batch = line_strip_builder.batch("radius variations");
            line_batch
                .add_segment_2d(glam::vec2(500.0, 10.0), glam::vec2(1000.0, 10.0))
                .radius(Size::new_scene(4.0))
                .color(Color32::from_rgb(255, 180, 1));
            line_batch
                .add_segment_2d(glam::vec2(500.0, 30.0), glam::vec2(1000.0, 30.0))
                .radius(Size::new_points(4.0))
                .color(Color32::from_rgb(255, 180, 1));
            line_batch
                .add_segment_2d(glam::vec2(500.0, 60.0), glam::vec2(1000.0, 60.0))
                .radius(Size::AUTO)
                .color(Color32::from_rgb(255, 180, 1));
            line_batch
                .add_segment_2d(glam::vec2(500.0, 90.0), glam::vec2(1000.0, 90.0))
                .radius(Size::AUTO_LARGE)
                .color(Color32::from_rgb(255, 180, 1));
        }

        // Points with different kinds of radius
        // The first two points are the same thickness if there no (!) scaling.
        // Moving the windows to a high dpi screen makes the second one bigger.
        // Also, it looks different under perspective projection.
        // The third point is automatic thickness which is determined by the point renderer implementation.
        let mut point_cloud_builder = PointCloudBuilder::new(re_ctx);
        point_cloud_builder.batch("points").add_points_2d(
            4,
            [
                glam::vec2(500.0, 120.0),
                glam::vec2(520.0, 120.0),
                glam::vec2(540.0, 120.0),
                glam::vec2(560.0, 120.0),
            ]
            .into_iter(),
            [
                Size::new_scene(4.0),
                Size::new_points(4.0),
                Size::AUTO,
                Size::AUTO_LARGE,
            ]
            .into_iter(),
            std::iter::repeat(Color32::from_rgb(55, 180, 1)),
            std::iter::repeat(re_renderer::PickingLayerInstanceId::default()),
        );

        // Pile stuff to test for overlap handling.
        // Do in individual batches to test depth offset.
        {
            let num_lines = 20_i16;
            let y_range = 700.0..780.0;

            // Cycle through which line is on top.
            let top_line = ((time.seconds_since_startup() * 6.0) as i16 % (num_lines * 2 - 1)
                - num_lines)
                .abs();
            for i in 0..num_lines {
                let depth_offset = if i < top_line { i } else { top_line * 2 - i };
                let mut batch = line_strip_builder
                    .batch(format!("overlapping objects {i}"))
                    .depth_offset(depth_offset);

                let x = 15.0 * i as f32 + 20.0;
                batch
                    .add_segment_2d(glam::vec2(x, y_range.start), glam::vec2(x, y_range.end))
                    .color(Hsva::new(0.25 / num_lines as f32 * i as f32, 1.0, 0.5, 1.0).into())
                    .radius(Size::new_points(10.0))
                    .flags(LineStripFlags::FLAG_COLOR_GRADIENT);
            }

            let num_points = 8;
            let size = Size::new_points(3.0);
            point_cloud_builder
                .batch("points overlapping with lines")
                .depth_offset(5)
                .add_points_2d(
                    num_points,
                    (0..num_points).map(|i| {
                        glam::vec2(
                            30.0 * i as f32 + 20.0,
                            y_range.start
                                + (y_range.end - y_range.start) / num_points as f32 * i as f32,
                        )
                    }),
                    std::iter::repeat(size),
                    std::iter::repeat(Color32::WHITE),
                    std::iter::repeat(re_renderer::PickingLayerInstanceId::default()),
                );
        }

        let line_strip_draw_data = line_strip_builder.to_draw_data(re_ctx).unwrap();
        let point_draw_data = point_cloud_builder.to_draw_data(re_ctx).unwrap();

        let image_scale = 4.0;
        let rectangle_draw_data = RectangleDrawData::new(
            re_ctx,
            &[
                TexturedRect {
                    top_left_corner_position: glam::vec3(500.0, 120.0, -0.05),
                    extent_u: self.rerun_logo_texture_width as f32 * image_scale * glam::Vec3::X,
                    extent_v: self.rerun_logo_texture_height as f32 * image_scale * glam::Vec3::Y,
                    colormapped_texture: ColormappedTexture::from_unorm_rgba(
                        self.rerun_logo_texture.clone(),
                    ),
                    options: RectangleOptions {
                        texture_filter_magnification: TextureFilterMag::Nearest,
                        texture_filter_minification: TextureFilterMin::Linear,
                        ..Default::default()
                    },
                },
                TexturedRect {
                    top_left_corner_position: glam::vec3(
                        500.0,
                        // Intentionally overlap pictures to illustrate z-fighting resolution
                        170.0 + self.rerun_logo_texture_height as f32 * image_scale * 0.25,
                        -0.05,
                    ),
                    extent_u: self.rerun_logo_texture_width as f32 * image_scale * glam::Vec3::X,
                    extent_v: self.rerun_logo_texture_height as f32 * image_scale * glam::Vec3::Y,
                    colormapped_texture: ColormappedTexture::from_unorm_rgba(
                        self.rerun_logo_texture.clone(),
                    ),
                    options: RectangleOptions {
                        texture_filter_magnification: TextureFilterMag::Linear,
                        texture_filter_minification: TextureFilterMin::Linear,
                        depth_offset: 1,
                        ..Default::default()
                    },
                },
            ],
        )
        .unwrap();

        vec![
            // 2d view to the left
            {
                let mut view_builder = ViewBuilder::new(
                    re_ctx,
                    TargetConfiguration {
                        name: "2D".into(),
                        resolution_in_pixel: splits[0].resolution_in_pixel,
                        view_from_world: macaw::IsoTransform::IDENTITY,
                        projection_from_view: Projection::Orthographic {
                            camera_mode:
                                view_builder::OrthographicCameraMode::TopLeftCornerAndExtendZ,
                            vertical_world_size: splits[0].resolution_in_pixel[1] as f32,
                            far_plane_distance: 1000.0,
                        },
                        pixels_from_point,
                        ..Default::default()
                    },
                );
                view_builder.queue_draw(&line_strip_draw_data);
                view_builder.queue_draw(&point_draw_data);
                view_builder.queue_draw(&rectangle_draw_data);
                let command_buffer = view_builder
                    .draw(re_ctx, ecolor::Rgba::TRANSPARENT)
                    .unwrap();
                framework::ViewDrawResult {
                    view_builder,
                    command_buffer,
                    target_location: splits[0].target_location,
                }
            },
            // and 3d view of the same scene to the right
            {
                let seconds_since_startup = time.seconds_since_startup();
                let camera_rotation_center = screen_size.extend(0.0) * 0.5;
                let camera_position = glam::vec3(
                    seconds_since_startup.sin(),
                    0.5,
                    seconds_since_startup.cos(),
                ) * screen_size.x.max(screen_size.y)
                    + camera_rotation_center;
                let mut view_builder = ViewBuilder::new(
                    re_ctx,
                    view_builder::TargetConfiguration {
                        name: "3D".into(),
                        resolution_in_pixel: splits[1].resolution_in_pixel,
                        view_from_world: macaw::IsoTransform::look_at_rh(
                            camera_position,
                            camera_rotation_center,
                            glam::Vec3::Y,
                        )
                        .unwrap(),
                        projection_from_view: Projection::Perspective {
                            vertical_fov: 70.0 * std::f32::consts::TAU / 360.0,
                            near_plane_distance: 0.01,
                            aspect_ratio: resolution[0] as f32 / resolution[1] as f32,
                        },
                        pixels_from_point,
                        ..Default::default()
                    },
                );
                let command_buffer = view_builder
                    .queue_draw(&line_strip_draw_data)
                    .queue_draw(&point_draw_data)
                    .queue_draw(&rectangle_draw_data)
                    .draw(re_ctx, ecolor::Rgba::TRANSPARENT)
                    .unwrap();
                framework::ViewDrawResult {
                    view_builder,
                    command_buffer,
                    target_location: splits[1].target_location,
                }
            },
        ]
    }

    fn on_keyboard_input(&mut self, _input: winit::event::KeyboardInput) {}
}

fn main() {
    framework::start::<Render2D>();
}
