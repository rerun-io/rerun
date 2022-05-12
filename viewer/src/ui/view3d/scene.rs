use super::camera::Camera;
use crate::{log_db::SpaceSummary, misc::ViewerContext};
use egui::Color32;
use egui::{util::hash, NumExt as _};
use glam::{vec3, Mat4, Quat, Vec3};
use itertools::Itertools as _;
use log_types::{Box3, Mesh3D};

pub struct Point {
    pub pos: [f32; 3],
    pub radius: f32,
    pub color: Color32,
}

pub struct LineSegments {
    pub segments: Vec<[[f32; 3]; 2]>,
    pub radius: f32,
    pub color: Color32,
}

pub enum MeshSourceData {
    Mesh3D(Mesh3D),
    /// e.g. the camera mesh
    StaticGlb(&'static [u8]),
}

pub struct MeshSource {
    pub mesh_id: u64,
    pub name: String,
    pub world_from_mesh: glam::Mat4,
    pub mesh_data: MeshSourceData,
}

#[derive(Default)]
pub struct Scene {
    pub points: Vec<Point>,
    pub line_segments: Vec<LineSegments>,
    pub meshes: Vec<MeshSource>,
}

impl Scene {
    #[allow(clippy::too_many_arguments)] // TODO: fewer arguments
    pub(crate) fn add_msg(
        &mut self,
        context: &ViewerContext,
        space_summary: &SpaceSummary,
        viewport_size: egui::Vec2,
        camera: &Camera,
        is_hovered: bool,
        color: Color32,
        msg: &log_types::LogMsg,
    ) {
        use log_types::*;

        let line_radius_in_points = (0.0005 * viewport_size.length()).at_least(1.5);
        let point_radius_in_points = 2.5 * line_radius_in_points;

        let radius_multiplier = if is_hovered { 1.5 } else { 1.0 };

        // Size of a pixel (in meters), when projected out one meter:
        let point_size_at_one_meter = camera.fov_y / viewport_size.y;

        let point_radius_from_distance =
            point_radius_in_points * point_size_at_one_meter * radius_multiplier;
        let line_radius_from_distance =
            line_radius_in_points * point_size_at_one_meter * radius_multiplier;

        let camera_plane = macaw::Plane3::from_normal_point(camera.forward(), camera.pos());

        match &msg.data {
            Data::Pos3(pos) => {
                // scale with distance
                let dist_to_camera = camera_plane.distance(Vec3::from(*pos));
                self.points.push(Point {
                    pos: *pos,
                    radius: dist_to_camera * point_radius_from_distance,
                    color,
                });
            }
            Data::Vec3(_) => {
                // Can't visualize vectors (yet)
            }
            Data::Box3(box3) => {
                self.add_box(camera, color, line_radius_from_distance, box3);
            }
            Data::Path3D(points) => {
                let bbox = macaw::BoundingBox::from_points(points.iter().copied().map(Vec3::from));
                let dist_to_camera = camera_plane.distance(bbox.center());
                let segments = points
                    .iter()
                    .tuple_windows()
                    .map(|(a, b)| [*a, *b])
                    .collect();

                self.line_segments.push(LineSegments {
                    segments,
                    radius: dist_to_camera * line_radius_from_distance,
                    color,
                });
            }
            Data::LineSegments3D(segments) => {
                let bbox = macaw::BoundingBox::from_points(
                    segments
                        .iter()
                        .flat_map(|&[a, b]| [Vec3::from(a), Vec3::from(b)]),
                );
                let dist_to_camera = camera_plane.distance(bbox.center());
                self.line_segments.push(LineSegments {
                    segments: segments.clone(),
                    radius: dist_to_camera * line_radius_from_distance,
                    color,
                });
            }
            Data::Mesh3D(mesh) => {
                self.meshes.push(MeshSource {
                    mesh_id: hash(msg.id),
                    name: msg.object_path.to_string(),
                    world_from_mesh: glam::Mat4::IDENTITY,
                    mesh_data: MeshSourceData::Mesh3D(mesh.clone()),
                });
            }
            Data::Camera(cam) => {
                let rotation = Quat::from_slice(&cam.rotation);
                let translation = Vec3::from_slice(&cam.position);

                // The camera mesh file is 1m long, looking down -Z, with X=right, Y=up.
                // The lens is at the origin.

                let dist_to_camera = camera_plane.distance(translation);

                if context.options.show_camera_mesh_in_3d {
                    let scale_based_on_scene_size =
                        radius_multiplier * 0.05 * space_summary.bbox3d.size().length();
                    let scale_based_on_distance =
                        dist_to_camera * point_radius_from_distance * 50.0; // shrink as we get very close. TODO: fade instead!
                    let scale = scale_based_on_scene_size.min(scale_based_on_distance);
                    let scale = Vec3::splat(scale);

                    let world_from_mesh =
                        Mat4::from_scale_rotation_translation(scale, rotation, translation);
                    self.meshes.push(MeshSource {
                        mesh_id: hash("camera"),
                        name: msg.object_path.to_string(),
                        world_from_mesh,
                        mesh_data: MeshSourceData::StaticGlb(include_bytes!(
                            "../../../data/camera.glb"
                        )),
                    });
                }

                if let (Some(intrinsis), Some([w, h])) = (cam.intrinsics, cam.resolution) {
                    // Frustum lines:
                    let world_from_cam = Mat4::from_rotation_translation(rotation, translation);
                    let intrinsis = glam::Mat3::from_cols_array_2d(&intrinsis);

                    // TODO: verify and clarify the coordinate systems! RHS, origin is what corner of image, etc.
                    let world_from_pixel = world_from_cam
                        * Mat4::from_diagonal([1.0, 1.0, -1.0, 1.0].into()) // negative Z, because we use RHS
                        * Mat4::from_mat3(intrinsis.inverse());

                    // At what distance do we end the frustum?
                    let d = space_summary.bbox3d.size().length() * 0.25;

                    let corners = [
                        world_from_pixel
                            .transform_point3(d * vec3(0.0, 0.0, 1.0))
                            .into(),
                        world_from_pixel
                            .transform_point3(d * vec3(0.0, h, 1.0))
                            .into(),
                        world_from_pixel
                            .transform_point3(d * vec3(w, h, 1.0))
                            .into(),
                        world_from_pixel
                            .transform_point3(d * vec3(w, 0.0, 1.0))
                            .into(),
                    ];

                    let center = translation.into();

                    let segments = vec![
                        [center, corners[0]],     // frustum corners
                        [center, corners[1]],     // frustum corners
                        [center, corners[2]],     // frustum corners
                        [center, corners[3]],     // frustum corners
                        [corners[0], corners[1]], // `d` distance plane sides
                        [corners[1], corners[2]], // `d` distance plane sides
                        [corners[2], corners[3]], // `d` distance plane sides
                        [corners[3], corners[0]], // `d` distance plane sides
                    ];

                    self.line_segments.push(LineSegments {
                        segments,
                        radius: dist_to_camera * line_radius_from_distance,
                        color: Color32::GRAY, // TODO
                    });
                }
            }
            _ => {
                debug_assert!(!msg.data.is_3d());
            }
        }
    }

    pub fn add_box(
        &mut self,
        camera: &Camera,
        color: Color32,
        line_radius_from_distance: f32,
        box3: &Box3,
    ) {
        let Box3 {
            rotation,
            translation,
            half_size,
        } = box3;
        let rotation = glam::Quat::from_array(*rotation);
        let translation = glam::Vec3::from(*translation);
        let half_size = glam::Vec3::from(*half_size);
        let transform =
            glam::Mat4::from_scale_rotation_translation(half_size, rotation, translation);

        let corners = [
            transform
                .transform_point3(vec3(-0.5, -0.5, -0.5))
                .to_array(),
            transform.transform_point3(vec3(-0.5, -0.5, 0.5)).to_array(),
            transform.transform_point3(vec3(-0.5, 0.5, -0.5)).to_array(),
            transform.transform_point3(vec3(-0.5, 0.5, 0.5)).to_array(),
            transform.transform_point3(vec3(0.5, -0.5, -0.5)).to_array(),
            transform.transform_point3(vec3(0.5, -0.5, 0.5)).to_array(),
            transform.transform_point3(vec3(0.5, 0.5, -0.5)).to_array(),
            transform.transform_point3(vec3(0.5, 0.5, 0.5)).to_array(),
        ];

        let segments = vec![
            // bottom:
            [corners[0b000], corners[0b001]],
            [corners[0b000], corners[0b010]],
            [corners[0b011], corners[0b001]],
            [corners[0b011], corners[0b010]],
            // top:
            [corners[0b100], corners[0b101]],
            [corners[0b100], corners[0b110]],
            [corners[0b111], corners[0b101]],
            [corners[0b111], corners[0b110]],
            // sides:
            [corners[0b000], corners[0b100]],
            [corners[0b001], corners[0b101]],
            [corners[0b010], corners[0b110]],
            [corners[0b011], corners[0b111]],
        ];

        let dist_to_camera = camera.pos().distance(translation);
        self.line_segments.push(LineSegments {
            segments,
            radius: dist_to_camera * line_radius_from_distance,
            color,
        });
    }
}
