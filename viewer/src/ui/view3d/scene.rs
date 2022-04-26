use super::camera::Camera;
use crate::log_db::SpaceSummary;
use egui::util::hash;
use egui::Color32;
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
    pub(crate) fn add_msg(
        &mut self,
        space_summary: &SpaceSummary,
        viewport_size: egui::Vec2,
        camera: &Camera,
        is_hovered: bool,
        color: Color32,
        msg: &log_types::LogMsg,
    ) {
        use log_types::*;

        let point_radius_in_points = 2.0;
        let line_radius_in_points = 1.0;

        let radius_multiplier = if is_hovered { 1.5 } else { 1.0 };

        // Size of a pixel (in meters), when projected out one meter:
        let point_size_at_one_meter = camera.fov_y / viewport_size.y;

        let point_radius_from_distance =
            point_radius_in_points * point_size_at_one_meter * radius_multiplier;
        let line_radius_from_distance =
            line_radius_in_points * point_size_at_one_meter * radius_multiplier;

        let eye_pos = camera.pos();

        match &msg.data {
            Data::Pos3(pos) => {
                // scale with distance
                let dist_to_camera = eye_pos.distance(Vec3::from(*pos));
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
                let dist_to_camera = eye_pos.distance(bbox.center());
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
                let dist_to_camera = eye_pos.distance(bbox.center());
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
            Data::Camera(camera) => {
                let rotation = Quat::from_slice(&camera.rotation);
                let translation = Vec3::from_slice(&camera.position);

                // camera mesh is 1m long in file
                let dist_to_camera = eye_pos.distance(translation);
                let scale_based_on_scene_size = 0.05 * space_summary.bbox3d.size().length();
                let scale_based_on_distance = dist_to_camera * point_radius_from_distance * 50.0; // shrink as we get very close
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
