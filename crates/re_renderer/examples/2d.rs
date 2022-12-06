use re_renderer::{
    renderer::{LineStripFlags, Rectangle, RectangleDrawData, TextureFilterMag, TextureFilterMin},
    resource_managers::{GpuTexture2DHandle, Texture2DCreationDesc},
    texture_values::ValueRgba8UnormSrgb,
    view_builder::{self, Projection, ViewBuilder},
    LineStripSeriesBuilder, Size,
};

mod framework;

struct Render2D {
    rerun_logo_texture: GpuTexture2DHandle,
    rerun_logo_texture_width: u32,
    rerun_logo_texture_height: u32,
}

impl framework::Example for Render2D {
    fn title() -> &'static str {
        "2D Rendering"
    }

    fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        let rerun_logo =
            image::load_from_memory(include_bytes!("../../re_viewer/data/logo_dark_mode.png"))
                .unwrap();

        let mut image_data = rerun_logo.as_rgba8().unwrap().to_vec();

        // Premultiply alpha (not doing any alpha blending, so this will look better on a black ground).
        for color in image_data.chunks_exact_mut(4) {
            color.clone_from_slice(
                &epaint::Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3])
                    .to_array(),
            );
        }

        let rerun_logo_texture = re_ctx.texture_manager_2d.create(
            &mut re_ctx.gpu_resources.textures,
            &Texture2DCreationDesc {
                label: "rerun logo".into(),
                data: &image_data,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                width: rerun_logo.width(),
                height: rerun_logo.height(),
            },
        );
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

        let mut line_strip_builder = LineStripSeriesBuilder::<()>::default();

        // Blue rect outline around the bottom right quarter.
        let line_radius = 10.0;
        let blue_rect_position = screen_size * 0.5 - glam::vec2(line_radius, line_radius);
        line_strip_builder
            .add_rectangle_outline_2d(
                blue_rect_position,
                glam::vec2(screen_size.x * 0.5, 0.0),
                glam::vec2(0.0, screen_size.y * 0.5),
            )
            .radius(Size::new_scene(line_radius))
            .color_rgb(50, 50, 255);

        // .. within, a orange rectangle
        line_strip_builder
            .add_rectangle_outline_2d(
                blue_rect_position + screen_size * 0.125,
                glam::vec2(screen_size.x * 0.25, 0.0),
                glam::vec2(0.0, screen_size.y * 0.25),
            )
            .radius(Size::new_scene(5.0))
            .color_rgb(255, 100, 1);

        // All variations of line caps
        for (i, flags) in [
            LineStripFlags::empty(),
            LineStripFlags::CAP_START_ROUND,
            LineStripFlags::CAP_END_ROUND,
            LineStripFlags::CAP_START_TRIANGLE,
            LineStripFlags::CAP_END_TRIANGLE,
            LineStripFlags::CAP_START_ROUND | LineStripFlags::CAP_END_ROUND,
            LineStripFlags::CAP_START_ROUND | LineStripFlags::CAP_END_TRIANGLE,
            LineStripFlags::CAP_START_TRIANGLE | LineStripFlags::CAP_END_ROUND,
            LineStripFlags::CAP_START_TRIANGLE | LineStripFlags::CAP_END_TRIANGLE,
        ]
        .iter()
        .enumerate()
        {
            let y = (i + 1) as f32 * 70.0;
            line_strip_builder
                .add_segment_2d(glam::vec2(70.0, y), glam::vec2(400.0, y))
                .radius(Size::new_scene(15.0))
                .flags(*flags);
        }

        // Lines with different kinds of radius
        // The first two lines are the same thickness if there no (!) scaling.
        // Moving the windows to a high dpi screen makes the second one bigger.
        // Also, it looks different under perspective projection.
        // The third line is automatic thickness which is determined by the line renderer implementation.
        line_strip_builder
            .add_segment_2d(glam::vec2(500.0, 10.0), glam::vec2(1000.0, 10.0))
            .radius(Size::new_scene(4.0))
            .color_rgb(255, 180, 1);
        line_strip_builder
            .add_segment_2d(glam::vec2(500.0, 30.0), glam::vec2(1000.0, 30.0))
            .radius(Size::new_points(4.0))
            .color_rgb(255, 180, 1);
        line_strip_builder
            .add_segment_2d(glam::vec2(500.0, 60.0), glam::vec2(1000.0, 60.0))
            .radius(Size::AUTO)
            .color_rgb(255, 180, 1);

        let line_strip_draw_data = line_strip_builder.to_draw_data(re_ctx);

        let image_scale = 4.0;
        let rectangle_draw_data = RectangleDrawData::new(
            re_ctx,
            &[
                Rectangle {
                    top_left_corner_position: glam::vec3(500.0, 100.0, -0.05),
                    extent_u: self.rerun_logo_texture_width as f32 * image_scale * glam::Vec3::X,
                    extent_v: self.rerun_logo_texture_height as f32 * image_scale * glam::Vec3::Y,
                    texture: self.rerun_logo_texture.clone(),
                    texture_filter_magnification: TextureFilterMag::Nearest,
                    texture_filter_minification: TextureFilterMin::Linear,
                    multiplicative_tint: re_renderer::ColorRgba8SrgbPremultiplied::WHITE,
                },
                Rectangle {
                    top_left_corner_position: glam::vec3(
                        500.0,
                        150.0 + self.rerun_logo_texture_height as f32 * image_scale,
                        -0.05,
                    ),
                    extent_u: self.rerun_logo_texture_width as f32 * image_scale * glam::Vec3::X,
                    extent_v: self.rerun_logo_texture_height as f32 * image_scale * glam::Vec3::Y,
                    texture: self.rerun_logo_texture.clone(),
                    texture_filter_magnification: TextureFilterMag::Linear,
                    texture_filter_minification: TextureFilterMin::Linear,
                    multiplicative_tint: re_renderer::ColorRgba8SrgbPremultiplied::WHITE,
                },
            ],
        )
        .unwrap();

        vec![
            // 2d view to the left
            {
                let mut view_builder = ViewBuilder::default();
                view_builder
                    .setup_view(
                        re_ctx,
                        view_builder::TargetConfiguration::new_2d_target(
                            "2D".into(),
                            splits[0].resolution_in_pixel,
                            1.0,
                            pixels_from_point,
                            glam::Vec2::ZERO,
                        ),
                    )
                    .unwrap();
                view_builder.queue_draw(&line_strip_draw_data);
                view_builder.queue_draw(&rectangle_draw_data);
                let command_buffer = view_builder
                    .draw(re_ctx, ValueRgba8UnormSrgb::TRANSPARENT)
                    .unwrap();
                framework::ViewDrawResult {
                    view_builder,
                    command_buffer,
                    target_location: splits[0].target_location,
                }
            },
            // and 3d view of the same scene to the right
            {
                {
                    let mut view_builder = ViewBuilder::default();
                    let seconds_since_startup = time.seconds_since_startup();
                    let camera_rotation_center = screen_size.extend(0.0) * 0.5;
                    let camera_position = glam::vec3(
                        seconds_since_startup.sin(),
                        0.5,
                        seconds_since_startup.cos(),
                    ) * screen_size.x.max(screen_size.y)
                        + camera_rotation_center;
                    view_builder
                        .setup_view(
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
                                },
                                pixels_from_point,
                            },
                        )
                        .unwrap();
                    view_builder.queue_draw(&line_strip_draw_data);
                    view_builder.queue_draw(&rectangle_draw_data);
                    let command_buffer = view_builder
                        .draw(re_ctx, ValueRgba8UnormSrgb::TRANSPARENT)
                        .unwrap();
                    framework::ViewDrawResult {
                        view_builder,
                        command_buffer,
                        target_location: splits[1].target_location,
                    }
                }
            },
        ]
    }

    fn on_keyboard_input(&mut self, _input: winit::event::KeyboardInput) {}
}

fn main() {
    framework::start::<Render2D>();
}
