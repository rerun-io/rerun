//! Depth offset and depth precision comparison test scene.
//!
//! Rectangles are at close distance to each other in the z==0 plane.
//! Rects on the left use "real" depth values, rects on the right use depth offset.
//! You should see the least saturated rects in front of more saturated rects.
//!
//! Press arrow up/down to increase/decrease the distance of the camera to the z==0 plane in tandem with the scale of the rectangles.
//! Press arrow left/right to increase/decrease the near plane distance.

use ecolor::Hsva;
use re_renderer::{
    renderer::{ColormappedTexture, RectangleDrawData, RectangleOptions, TexturedRect},
    view_builder::{self, Projection, ViewBuilder},
};

mod framework;

struct Render2D {
    distance_scale: f32,
    near_plane: f32,
}

impl framework::Example for Render2D {
    fn title() -> &'static str {
        "Depth Offset"
    }

    fn new(_re_ctx: &mut re_renderer::RenderContext) -> Self {
        Render2D {
            distance_scale: 100.0,
            near_plane: 0.1,
        }
    }

    fn draw(
        &mut self,
        re_ctx: &mut re_renderer::RenderContext,
        resolution: [u32; 2],
        _time: &framework::Time,
        pixels_from_point: f32,
    ) -> Vec<framework::ViewDrawResult> {
        let mut rectangles = Vec::new();

        let extent_u = glam::vec3(1.0, 0.0, 0.0) * self.distance_scale;
        let extent_v = glam::vec3(0.0, 1.0, 0.0) * self.distance_scale;

        // Rectangles on the left from near to far, using z.
        let base_top_left = glam::vec2(-0.8, -0.5) * self.distance_scale
            - (extent_u.truncate() + extent_v.truncate()) * 0.5;
        let xy_step = glam::vec2(-0.1, 0.1) * self.distance_scale;
        let z_values = [0.1, 0.01, 0.001, 0.0001, 0.00001, 0.0]; // Make sure to go from near to far so that painter's algorithm would fail if depth values are no longer distinct.
        for (i, z) in z_values.into_iter().enumerate() {
            let saturation = 0.1 + i as f32 / z_values.len() as f32 * 0.9;
            rectangles.push(TexturedRect {
                top_left_corner_position: (base_top_left + i as f32 * xy_step).extend(z),
                extent_u,
                extent_v,
                colormapped_texture: ColormappedTexture::from_unorm_rgba(
                    re_ctx
                        .texture_manager_2d
                        .white_texture_unorm_handle()
                        .clone(),
                ),
                options: RectangleOptions {
                    multiplicative_tint: Hsva::new(0.0, saturation, 0.5, 1.0).into(),
                    ..Default::default()
                },
            });
        }

        // Rectangles on the right from near to far, using depth offset.
        let base_top_left = glam::vec2(0.8, -0.5) * self.distance_scale
            - (extent_u.truncate() + extent_v.truncate()) * 0.5;
        let xy_step = glam::vec2(0.1, 0.1) * self.distance_scale;
        let depth_offsets = [1000, 100, 10, 1, 0, -1]; // Make sure to go from near to far so that painter's algorithm would fail if depth values are no longer distinct.
        for (i, depth_offset) in depth_offsets.into_iter().enumerate() {
            let saturation = 0.1 + i as f32 / depth_offsets.len() as f32 * 0.9;
            rectangles.push(TexturedRect {
                top_left_corner_position: (base_top_left + i as f32 * xy_step).extend(0.0),
                extent_u,
                extent_v,
                colormapped_texture: ColormappedTexture::from_unorm_rgba(
                    re_ctx
                        .texture_manager_2d
                        .white_texture_unorm_handle()
                        .clone(),
                ),
                options: RectangleOptions {
                    multiplicative_tint: Hsva::new(0.68, saturation, 0.5, 1.0).into(),
                    depth_offset,
                    ..Default::default()
                },
            });
        }

        let mut view_builder = ViewBuilder::new(
            re_ctx,
            view_builder::TargetConfiguration {
                name: "3D".into(),
                resolution_in_pixel: resolution,
                view_from_world: macaw::IsoTransform::look_at_rh(
                    glam::Vec3::Z * 2.0 * self.distance_scale,
                    glam::Vec3::ZERO,
                    glam::Vec3::Y,
                )
                .unwrap(),
                projection_from_view: Projection::Perspective {
                    vertical_fov: 70.0 * std::f32::consts::TAU / 360.0,
                    near_plane_distance: self.near_plane,
                    aspect_ratio: resolution[0] as f32 / resolution[1] as f32,
                },
                pixels_from_point,
                ..Default::default()
            },
        );
        let command_buffer = view_builder
            .queue_draw(&RectangleDrawData::new(re_ctx, &rectangles).unwrap())
            .draw(re_ctx, ecolor::Rgba::TRANSPARENT)
            .unwrap();

        vec![{
            framework::ViewDrawResult {
                view_builder,
                command_buffer,
                target_location: glam::Vec2::ZERO,
            }
        }]
    }

    fn on_keyboard_input(&mut self, input: winit::event::KeyboardInput) {
        if input.state == winit::event::ElementState::Pressed {
            match input.virtual_keycode {
                Some(winit::event::VirtualKeyCode::Up) => {
                    self.distance_scale *= 1.1;
                    re_log::info!(self.distance_scale);
                }
                Some(winit::event::VirtualKeyCode::Down) => {
                    self.distance_scale /= 1.1;
                    re_log::info!(self.distance_scale);
                }
                Some(winit::event::VirtualKeyCode::Right) => {
                    self.near_plane *= 1.1;
                    re_log::info!(self.near_plane);
                }
                Some(winit::event::VirtualKeyCode::Left) => {
                    self.near_plane /= 1.1;
                    re_log::info!(self.near_plane);
                }
                _ => {}
            }
        }
    }
}

fn main() {
    framework::start::<Render2D>();
}
