use std::sync::Arc;

use egui::Color32;
use egui::{util::hash, NumExt as _};
use glam::{vec3, Mat4, Quat, Vec3};
use itertools::Itertools as _;
use log_types::{Box3, LogId, Mesh3D};

use crate::{
    log_db::SpaceSummary, math::line_segment_distance_sq_to_point, misc::mesh_loader::CpuMesh,
    misc::ViewerContext,
};

use super::camera::Camera;

pub struct Point {
    pub log_id: LogId,
    pub pos: [f32; 3],
    pub radius: f32,
    pub color: Color32,
}

pub struct LineSegments {
    pub log_id: LogId,
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
    pub log_id: LogId,
    pub mesh_id: u64,
    pub world_from_mesh: glam::Mat4,
    pub cpu_mesh: Arc<CpuMesh>,
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
        context: &mut ViewerContext,
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
                    log_id: msg.id,
                    pos: *pos,
                    radius: dist_to_camera * point_radius_from_distance,
                    color,
                });
            }
            Data::Vec3(_) => {
                // Can't visualize vectors (yet)
            }
            Data::Box3(box3) => {
                self.add_box(msg.id, camera, color, line_radius_from_distance, box3);
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
                    log_id: msg.id,
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
                    log_id: msg.id,
                    segments: segments.clone(),
                    radius: dist_to_camera * line_radius_from_distance,
                    color,
                });
            }
            Data::Mesh3D(mesh) => {
                let mesh_id = hash(msg.id);
                if let Some(cpu_mesh) = context.cpu_mesh_cache.load(
                    mesh_id,
                    &msg.object_path.to_string(),
                    &MeshSourceData::Mesh3D(mesh.clone()),
                ) {
                    self.meshes.push(MeshSource {
                        log_id: msg.id,
                        mesh_id,
                        world_from_mesh: glam::Mat4::IDENTITY,
                        cpu_mesh,
                    });
                }
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

                    let mesh_id = hash("camera_mesh");
                    let world_from_mesh =
                        Mat4::from_scale_rotation_translation(scale, rotation, translation);

                    if let Some(cpu_mesh) = context.cpu_mesh_cache.load(
                        mesh_id,
                        &msg.object_path.to_string(),
                        &MeshSourceData::StaticGlb(include_bytes!("../../../data/camera.glb")),
                    ) {
                        self.meshes.push(MeshSource {
                            log_id: msg.id,
                            mesh_id,
                            world_from_mesh,
                            cpu_mesh,
                        });
                    }
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
                        log_id: msg.id,
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
        id: LogId,
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
            log_id: id,
            segments,
            radius: dist_to_camera * line_radius_from_distance,
            color,
        });
    }

    pub fn picking(&self, ui: &egui::Ui, rect: &egui::Rect, camera: &Camera) -> Option<LogId> {
        crate::profile_function!();
        let pointer_pos = ui.ctx().pointer_hover_pos()?;
        let screen_from_world = camera.screen_from_world(rect);
        let world_from_screen = screen_from_world.inverse();
        let ray_in_screen =
            macaw::Ray3::from_origin_dir(Vec3::ZERO, Vec3::new(pointer_pos.x, pointer_pos.y, -1.0));
        let ray_in_world = world_from_screen * ray_in_screen;

        let Self {
            points,
            line_segments,
            meshes,
        } = self;

        // in points
        let max_side_dist_sq = 5.0 * 5.0; // TODO: interaction radius from egui

        // in points
        let mut closest_side_dist_sq = max_side_dist_sq;
        // meters along the ray
        let mut closest_t = f32::INFINITY;
        let mut closest_id = None;

        for point in points {
            let screen_pos = screen_from_world.project_point3(point.pos.into());
            if screen_pos.z < 0.0 {
                continue; // TODO: don't we expect negative Z!? RHS etc
            }
            let dist_sq = egui::pos2(screen_pos.x, screen_pos.y).distance_sq(pointer_pos);
            if dist_sq < max_side_dist_sq {
                let t = screen_pos.z.abs();
                if t < closest_t || dist_sq < closest_side_dist_sq {
                    closest_t = t;
                    closest_side_dist_sq = dist_sq;
                    closest_id = Some(point.log_id);
                }
            }
        }

        for line_segments in line_segments {
            use egui::pos2;

            for [a, b] in &line_segments.segments {
                let a = screen_from_world.transform_point3((*a).into());
                let b = screen_from_world.transform_point3((*b).into());
                let dist_sq = line_segment_distance_sq_to_point(
                    [pos2(a.x, a.y), pos2(b.x, b.y)],
                    pointer_pos,
                );

                if dist_sq < max_side_dist_sq {
                    let t = a.z.abs(); // not very accurate
                    if t < closest_t || dist_sq < closest_side_dist_sq {
                        closest_t = t;
                        closest_side_dist_sq = dist_sq;
                        closest_id = Some(line_segments.log_id);
                    }
                }
            }
        }

        for mesh in meshes {
            let ray_in_mesh = mesh.world_from_mesh.inverse() * ray_in_world;
            let t = crate::math::ray_bbox_intersect(&ray_in_mesh, mesh.cpu_mesh.bbox());
            if t < f32::INFINITY {
                let dist_sq = 0.0;
                if t < closest_t || dist_sq < closest_side_dist_sq {
                    closest_t = t;
                    closest_side_dist_sq = dist_sq;
                    closest_id = Some(mesh.log_id);
                }
            }
        }

        closest_id
    }
}
