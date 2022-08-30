use std::sync::Arc;

use egui::util::hash;
use glam::{vec3, Vec3};
use itertools::Itertools as _;

use re_log_types::{Box3, Mesh3D, ObjPath, ObjPathHash};

use crate::{
    math::line_segment_distance_sq_to_point_2d, misc::mesh_loader::CpuMesh, misc::ViewerContext,
};

use super::eye::Eye;

pub struct Point {
    pub obj_path_hash: ObjPathHash,
    pub pos: [f32; 3],
    pub radius: f32,
    pub color: [u8; 4],
}

pub struct LineSegments {
    pub obj_path_hash: ObjPathHash,
    pub segments: Vec<[[f32; 3]; 2]>,
    pub radius: f32,
    pub color: [u8; 4],
}

pub enum MeshSourceData {
    Mesh3D(Mesh3D),
    /// e.g. the camera mesh
    StaticGlb(&'static [u8]),
}

pub struct MeshSource {
    pub obj_path_hash: ObjPathHash,
    pub mesh_id: u64,
    pub world_from_mesh: glam::Affine3A,
    pub cpu_mesh: Arc<CpuMesh>,
}

#[derive(Default)]
pub struct Scene {
    pub points: Vec<Point>,
    pub line_segments: Vec<LineSegments>,
    pub meshes: Vec<MeshSource>,

    /// Multiply this with the distance to a point to get its suggested radius.
    pub point_radius_from_distance: f32,
    /// Multiply this with the distance to a line to get its suggested radius.
    pub line_radius_from_distance: f32,
}

impl Scene {
    pub(crate) fn from_objects(
        ctx: &mut ViewerContext<'_>,
        scene_bbox: &macaw::BoundingBox,
        viewport_size: egui::Vec2,
        eye: &Eye,
        hovered_obj: Option<&ObjPath>,
        objects: &re_data_store::Objects<'_>,
    ) -> Self {
        crate::profile_function!();
        let hovered_obj_hash = hovered_obj.map_or(ObjPathHash::NONE, |obj| *obj.hash());

        // HACK because three-d handles colors wrong. TODO(emilk): fix three-d
        let gamma_lut = (0..=255)
            .map(|c| ((c as f32 / 255.0).powf(2.2) * 255.0).round() as u8)
            .collect_vec();
        let gamma_lut = &gamma_lut[0..256]; // saves us bounds checks later.

        let boost_size_on_hover = |props: &re_data_store::ObjectProps<'_>, radius: f32| {
            if Some(props.obj_path) == hovered_obj {
                1.5 * radius
            } else {
                radius
            }
        };
        let object_color = |ctx: &mut ViewerContext<'_>, props: &re_data_store::ObjectProps<'_>| {
            let [r, g, b, a] = if *props.obj_path.hash() == hovered_obj_hash {
                [255; 4]
            } else if let Some(color) = props.color {
                color
            } else {
                let [r, g, b] = ctx.random_color(props);
                [r, g, b, 255]
            };

            let r = gamma_lut[r as usize];
            let g = gamma_lut[g as usize];
            let b = gamma_lut[b as usize];
            [r, g, b, a]
        };

        let viewport_area = viewport_size.x * viewport_size.y;

        let line_radius_in_points = (0.0005 * viewport_size.length()).clamp(1.5, 5.0);

        // More points -> smaller points
        let point_radius_in_points =
            (0.3 * (viewport_area / (objects.point3d.len() + 1) as f32).sqrt()).clamp(0.1, 5.0);

        // Size of a pixel (in meters), when projected out one meter:
        let point_size_at_one_meter = eye.fov_y / viewport_size.y;

        let point_radius_from_distance = point_radius_in_points * point_size_at_one_meter;
        let line_radius_from_distance = line_radius_in_points * point_size_at_one_meter;

        let eye_camera_plane = macaw::Plane3::from_normal_point(eye.forward(), eye.pos());

        let mut scene = Scene {
            point_radius_from_distance,
            line_radius_from_distance,
            ..Default::default()
        };

        {
            crate::profile_scope!("point3d");
            scene.points.reserve(objects.point3d.len());
            for (props, obj) in objects.point3d.iter() {
                let re_data_store::Point3D { pos, radius } = *obj;

                let dist_to_eye = eye_camera_plane.distance(Vec3::from(*pos));
                let radius = radius.unwrap_or(dist_to_eye * point_radius_from_distance);
                let radius = boost_size_on_hover(props, radius);

                scene.points.push(Point {
                    obj_path_hash: *props.obj_path.hash(),
                    pos: *pos,
                    radius,
                    color: object_color(ctx, props),
                });
            }
        }

        {
            crate::profile_scope!("box3d");
            for (props, obj) in objects.box3d.iter() {
                let re_data_store::Box3D { obb, stroke_width } = obj;
                let line_radius = stroke_width.map_or_else(
                    || {
                        let dist_to_eye =
                            eye_camera_plane.distance(glam::Vec3::from(obb.translation));
                        dist_to_eye * line_radius_from_distance
                    },
                    |w| w / 2.0,
                );
                let line_radius = boost_size_on_hover(props, line_radius);
                let color = object_color(ctx, props);
                scene.add_box(props.obj_path, color, line_radius, obb);
            }
        }

        {
            crate::profile_scope!("path3d");
            for (props, obj) in objects.path3d.iter() {
                let re_data_store::Path3D {
                    points,
                    stroke_width,
                } = obj;

                let line_radius = stroke_width.map_or_else(
                    || {
                        let bbox =
                            macaw::BoundingBox::from_points(points.iter().copied().map(Vec3::from));
                        let dist_to_eye = eye_camera_plane.distance(bbox.center());
                        dist_to_eye * line_radius_from_distance
                    },
                    |w| w / 2.0,
                );
                let line_radius = boost_size_on_hover(props, line_radius);
                let color = object_color(ctx, props);

                let segments = points
                    .iter()
                    .tuple_windows()
                    .map(|(a, b)| [*a, *b])
                    .collect();

                scene.line_segments.push(LineSegments {
                    obj_path_hash: *props.obj_path.hash(),
                    segments,
                    radius: line_radius,
                    color,
                });
            }
        }

        {
            crate::profile_scope!("line_segments3d");
            for (props, obj) in objects.line_segments3d.iter() {
                let re_data_store::LineSegments3D {
                    points,
                    stroke_width,
                } = *obj;

                let line_radius = stroke_width.map_or_else(
                    || {
                        let bbox =
                            macaw::BoundingBox::from_points(points.iter().copied().map(Vec3::from));
                        let dist_to_eye = eye_camera_plane.distance(bbox.center());
                        dist_to_eye * line_radius_from_distance
                    },
                    |w| w / 2.0,
                );
                let line_radius = boost_size_on_hover(props, line_radius);
                let color = object_color(ctx, props);

                scene.line_segments.push(LineSegments {
                    obj_path_hash: *props.obj_path.hash(),
                    segments: bytemuck::allocation::pod_collect_to_vec(points),
                    radius: line_radius,
                    color,
                });
            }
        }

        {
            crate::profile_scope!("mesh3d");
            for (props, obj) in objects.mesh3d.iter() {
                let re_data_store::Mesh3D { mesh } = *obj;
                let mesh_id = hash(props.msg_id);
                if let Some(cpu_mesh) = ctx.cache.cpu_mesh.load(
                    mesh_id,
                    &props.obj_path.to_string(),
                    &MeshSourceData::Mesh3D(mesh.clone()),
                ) {
                    // TODO(emilk): props.color
                    scene.meshes.push(MeshSource {
                        obj_path_hash: *props.obj_path.hash(),
                        mesh_id,
                        world_from_mesh: glam::Affine3A::IDENTITY,
                        cpu_mesh,
                    });
                }
            }
        }

        {
            crate::profile_scope!("camera");
            for (props, obj) in objects.camera.iter() {
                let re_data_store::Camera { camera } = *obj;

                let world_from_view = crate::misc::cam::world_from_view(camera);

                let dist_to_eye = eye_camera_plane.distance(world_from_view.translation());
                let color = object_color(ctx, props);

                let scale_based_on_scene_size = 0.05 * scene_bbox.size().length();
                let scale_based_on_distance = dist_to_eye * point_radius_from_distance * 50.0; // shrink as we get very close. TODO(emilk): fade instead!
                let scale = scale_based_on_scene_size.min(scale_based_on_distance);
                let scale = boost_size_on_hover(props, scale);

                if ctx.options.show_camera_mesh_in_3d {
                    // The camera mesh file is 1m long, looking down -Z, with X=right, Y=up.
                    // The lens is at the origin.

                    let scale = Vec3::splat(scale);

                    let mesh_id = hash("camera_mesh");
                    let world_from_mesh = world_from_view * glam::Affine3A::from_scale(scale);

                    if let Some(cpu_mesh) = ctx.cache.cpu_mesh.load(
                        mesh_id,
                        "camera_mesh",
                        &MeshSourceData::StaticGlb(include_bytes!("../../../data/camera.glb")),
                    ) {
                        scene.meshes.push(MeshSource {
                            obj_path_hash: *props.obj_path.hash(),
                            mesh_id,
                            world_from_mesh,
                            cpu_mesh,
                        });
                    }
                }

                if ctx.options.show_camera_axes_in_3d {
                    let world_from_view = crate::misc::cam::world_from_view(camera);
                    let center = world_from_view.translation();
                    let radius = dist_to_eye * line_radius_from_distance * 2.0;

                    for (axis_index, dir) in camera
                        .camera_space_convention
                        .axis_dirs_in_rerun_view_space()
                        .iter()
                        .enumerate()
                    {
                        let color = axis_color(axis_index);
                        let axis_end =
                            world_from_view.transform_point3(scale * glam::Vec3::from(*dir));
                        scene.line_segments.push(LineSegments {
                            obj_path_hash: *props.obj_path.hash(),
                            segments: vec![[center.into(), axis_end.into()]],
                            radius,
                            color,
                        });
                    }
                }

                let line_radius = dist_to_eye * line_radius_from_distance;
                scene.add_camera_frustum(camera, scene_bbox, props.obj_path, line_radius, color);
            }
        }

        scene
    }

    /// Paint frustum lines
    fn add_camera_frustum(
        &mut self,
        cam: &re_log_types::Camera,
        scene_bbox: &macaw::BoundingBox,
        obj_path: &ObjPath,
        line_radius: f32,
        color: [u8; 4],
    ) {
        if let (Some(world_from_pixel), Some([w, h])) =
            (crate::misc::cam::world_from_pixel(cam), cam.resolution)
        {
            let world_from_view = crate::misc::cam::world_from_view(cam);

            // At what distance do we end the frustum?
            let d = scene_bbox.size().length() * 0.3;

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

            let center = world_from_view.translation().into();

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
                obj_path_hash: *obj_path.hash(),
                segments,
                radius: line_radius,
                color,
            });
        }
    }

    fn add_box(&mut self, obj_path: &ObjPath, color: [u8; 4], line_radius: f32, box3: &Box3) {
        let Box3 {
            rotation,
            translation,
            half_size,
        } = box3;
        let rotation = glam::Quat::from_array(*rotation);
        let translation = glam::Vec3::from(*translation);
        let half_size = glam::Vec3::from(*half_size);
        let transform =
            glam::Affine3A::from_scale_rotation_translation(half_size, rotation, translation);

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

        self.line_segments.push(LineSegments {
            obj_path_hash: *obj_path.hash(),
            segments,
            radius: line_radius,
            color,
        });
    }

    pub fn picking(
        &self,
        pointer_pos: egui::Pos2,
        rect: &egui::Rect,
        eye: &Eye,
    ) -> Option<(ObjPathHash, glam::Vec3)> {
        crate::profile_function!();

        let screen_from_world = eye.screen_from_world(rect);
        let world_from_screen = screen_from_world.inverse();
        let ray_dir =
            world_from_screen.project_point3(Vec3::new(pointer_pos.x, pointer_pos.y, -1.0))
                - eye.pos();
        let ray_in_world = macaw::Ray3::from_origin_dir(eye.pos(), ray_dir.normalize());

        let Self {
            points,
            line_segments,
            meshes,
            point_radius_from_distance: _,
            line_radius_from_distance: _,
        } = self;

        // in points
        let max_side_dist_sq = 5.0 * 5.0; // TODO(emilk): interaction radius from egui

        let mut closest_z = f32::INFINITY;
        // in points
        let mut closest_side_dist_sq = max_side_dist_sq;
        let mut closest_obj_path_hash = None;

        {
            crate::profile_scope!("points");
            for point in points {
                if point.obj_path_hash != ObjPathHash::NONE {
                    // TODO(emilk): take point radius into account
                    let screen_pos = screen_from_world.project_point3(point.pos.into());
                    if screen_pos.z < 0.0 {
                        continue; // TODO(emilk): don't we expect negative Z!? RHS etc
                    }
                    let dist_sq = egui::pos2(screen_pos.x, screen_pos.y).distance_sq(pointer_pos);
                    if dist_sq < max_side_dist_sq {
                        let t = screen_pos.z.abs();
                        if t < closest_z || dist_sq < closest_side_dist_sq {
                            closest_z = t;
                            closest_side_dist_sq = dist_sq;
                            closest_obj_path_hash = Some(point.obj_path_hash);
                        }
                    }
                }
            }
        }

        {
            crate::profile_scope!("line_segments");
            for line_segments in line_segments {
                if line_segments.obj_path_hash != ObjPathHash::NONE {
                    // TODO(emilk): take line segment radius into account
                    use egui::pos2;

                    for [a, b] in &line_segments.segments {
                        let a = screen_from_world.project_point3((*a).into());
                        let b = screen_from_world.project_point3((*b).into());
                        let dist_sq = line_segment_distance_sq_to_point_2d(
                            [pos2(a.x, a.y), pos2(b.x, b.y)],
                            pointer_pos,
                        );

                        if dist_sq < max_side_dist_sq {
                            let t = a.z.abs(); // not very accurate
                            if t < closest_z || dist_sq < closest_side_dist_sq {
                                closest_z = t;
                                closest_side_dist_sq = dist_sq;
                                closest_obj_path_hash = Some(line_segments.obj_path_hash);
                            }
                        }
                    }
                }
            }
        }

        {
            crate::profile_scope!("meshes");
            for mesh in meshes {
                if mesh.obj_path_hash != ObjPathHash::NONE {
                    let ray_in_mesh = (mesh.world_from_mesh.inverse() * ray_in_world).normalize();
                    let t = crate::math::ray_bbox_intersect(&ray_in_mesh, mesh.cpu_mesh.bbox());

                    if t < f32::INFINITY {
                        let dist_sq = 0.0;
                        if t < closest_z || dist_sq < closest_side_dist_sq {
                            closest_z = t; // TODO(emilk): I think this is wrong
                            closest_side_dist_sq = dist_sq;
                            closest_obj_path_hash = Some(mesh.obj_path_hash);
                        }
                    }
                }
            }
        }

        if let Some(closest_obj_path_hash) = closest_obj_path_hash {
            let closest_point = world_from_screen.project_point3(Vec3::new(
                pointer_pos.x,
                pointer_pos.y,
                closest_z,
            ));
            Some((closest_obj_path_hash, closest_point))
        } else {
            None
        }
    }
}

fn axis_color(axis: usize) -> [u8; 4] {
    match axis {
        0 => [255, 25, 25, 255],
        1 => [0, 240, 0, 255],
        2 => [80, 80, 255, 255],
        _ => unreachable!("Axis should be one of 0,1,2; got {axis}"),
    }
}
