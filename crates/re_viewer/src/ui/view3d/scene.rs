#[cfg(feature = "glow")]
use egui::util::hash;
use glam::{vec3, Vec3};
use itertools::Itertools as _;
use std::sync::Arc;

use re_data_store::{InstanceId, InstanceIdHash};

use crate::{math::line_segment_distance_sq_to_point_2d, misc::ViewerContext};

#[cfg(feature = "glow")]
use crate::misc::mesh_loader::CpuMesh;

// TODO(andreas): Dummy for disabling glow. Need a threed independent mesh obv.
#[cfg(not(feature = "glow"))]
pub struct CpuMesh {
    bbox: macaw::BoundingBox,
}
#[cfg(not(feature = "glow"))]
impl CpuMesh {
    pub fn bbox(&self) -> &macaw::BoundingBox {
        &self.bbox
    }
}

use super::eye::Eye;

pub struct Point {
    pub instance_id: InstanceIdHash,
    pub pos: [f32; 3],
    pub radius: f32,
    pub color: [u8; 4],
}

pub struct LineSegments {
    pub instance_id: InstanceIdHash,
    pub segments: Vec<[[f32; 3]; 2]>,
    pub radius: f32,
    pub color: [u8; 4],
}

#[cfg(feature = "glow")]
pub enum MeshSourceData {
    Mesh3D(re_log_types::Mesh3D),
    /// e.g. the camera mesh
    StaticGlb(&'static [u8]),
}

pub struct MeshSource {
    pub instance_id: InstanceIdHash,
    pub mesh_id: u64,
    pub world_from_mesh: glam::Affine3A,
    pub cpu_mesh: Arc<CpuMesh>,
    pub tint: Option<[u8; 4]>,
}

pub struct Label {
    pub(crate) text: String,
    /// Origin of the label
    pub(crate) origin: Vec3,
}

#[derive(Default)]
pub struct Scene {
    pub points: Vec<Point>,
    pub line_segments: Vec<LineSegments>,
    pub meshes: Vec<MeshSource>,
    pub labels: Vec<Label>,

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
        hovered_instance_id: Option<&InstanceId>,
        objects: &re_data_store::Objects<'_>,
    ) -> Self {
        crate::profile_function!();
        let hovered_instance_id_hash =
            hovered_instance_id.map_or(InstanceIdHash::NONE, |id| id.hash());

        // hack because three-d handles colors wrong. TODO(emilk): fix three-d
        let gamma_lut = (0..=255)
            .map(|c| ((c as f32 / 255.0).powf(2.2) * 255.0).round() as u8)
            .collect_vec();
        let gamma_lut = &gamma_lut[0..256]; // saves us bounds checks later.

        let boost_size_on_hover = |props: &re_data_store::InstanceProps<'_>, radius: f32| {
            if hovered_instance_id_hash.is_instance(props) {
                1.5 * radius
            } else {
                radius
            }
        };
        let object_color = |ctx: &mut ViewerContext<'_>,
                            props: &re_data_store::InstanceProps<'_>| {
            let [r, g, b, a] = if hovered_instance_id_hash.is_instance(props) {
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

        let eye_camera_plane =
            macaw::Plane3::from_normal_point(eye.forward_in_world(), eye.pos_in_world());

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
                    instance_id: InstanceIdHash::from_props(props),
                    pos: *pos,
                    radius,
                    color: object_color(ctx, props),
                });
            }
        }

        {
            crate::profile_scope!("box3d");
            for (props, obj) in objects.box3d.iter() {
                let re_data_store::Box3D {
                    obb,
                    stroke_width,
                    label,
                } = obj;
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
                scene.add_box(
                    InstanceIdHash::from_props(props),
                    color,
                    line_radius,
                    *label,
                    obb,
                );
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
                    instance_id: InstanceIdHash::from_props(props),
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
                    instance_id: InstanceIdHash::from_props(props),
                    segments: bytemuck::allocation::pod_collect_to_vec(points),
                    radius: line_radius,
                    color,
                });
            }
        }

        #[cfg(feature = "glow")]
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
                        instance_id: InstanceIdHash::from_props(props),
                        mesh_id,
                        world_from_mesh: glam::Affine3A::IDENTITY,
                        cpu_mesh,
                        tint: None,
                    });
                }
            }
        }

        #[cfg(feature = "glow")]
        {
            crate::profile_scope!("arrow3d");
            for (props, obj) in objects.arrow3d.iter() {
                let re_data_store::Arrow3D {
                    arrow,
                    label,
                    width_scale,
                } = obj;
                let width = boost_size_on_hover(props, width_scale.unwrap_or(1.0));
                let color = object_color(ctx, props);
                let instance_id = InstanceIdHash::from_props(props);
                scene.add_arrow(ctx, instance_id, color, Some(width), *label, arrow);
            }
        }

        {
            crate::profile_scope!("camera");
            for (props, obj) in objects.camera.iter() {
                let re_data_store::Camera { camera } = *obj;

                let instance_id = InstanceIdHash::from_props(props);

                let world_from_view = crate::misc::cam::world_from_view(&camera.extrinsics);

                let dist_to_eye = eye_camera_plane.distance(world_from_view.translation());
                let color = object_color(ctx, props);

                let scale_based_on_scene_size = 0.05 * scene_bbox.size().length();
                let scale_based_on_distance = dist_to_eye * point_radius_from_distance * 50.0; // shrink as we get very close. TODO(emilk): fade instead!
                let scale = scale_based_on_scene_size.min(scale_based_on_distance);
                let scale = boost_size_on_hover(props, scale);

                #[cfg(feature = "glow")]
                {
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
                                instance_id,
                                mesh_id,
                                world_from_mesh,
                                cpu_mesh,
                                tint: None,
                            });
                        }
                    }
                }

                if ctx.options.show_camera_axes_in_3d {
                    let center = world_from_view.translation();
                    let radius = dist_to_eye * line_radius_from_distance * 2.0;

                    for (axis_index, dir) in camera
                        .extrinsics
                        .camera_space_convention
                        .axis_dirs_in_rerun_view_space()
                        .iter()
                        .enumerate()
                    {
                        let color = axis_color(axis_index);
                        let axis_end =
                            world_from_view.transform_point3(scale * glam::Vec3::from(*dir));
                        scene.line_segments.push(LineSegments {
                            instance_id,
                            segments: vec![[center.into(), axis_end.into()]],
                            radius,
                            color,
                        });
                    }
                }

                let line_radius = dist_to_eye * line_radius_from_distance;
                scene.add_camera_frustum(camera, scene_bbox, instance_id, line_radius, color);
            }
        }

        scene
    }

    /// Paint frustum lines
    fn add_camera_frustum(
        &mut self,
        cam: &re_log_types::Camera,
        scene_bbox: &macaw::BoundingBox,
        instance_id: InstanceIdHash,
        line_radius: f32,
        color: [u8; 4],
    ) {
        if let (Some(world_from_image), Some(intrinsics)) =
            (crate::misc::cam::world_from_image(cam), cam.intrinsics)
        {
            let [w, h] = intrinsics.resolution;

            let world_from_view = crate::misc::cam::world_from_view(&cam.extrinsics);

            // At what distance do we end the frustum?
            let d = scene_bbox.size().length() * 0.3;

            let corners = [
                world_from_image
                    .transform_point3(d * vec3(0.0, 0.0, 1.0))
                    .into(),
                world_from_image
                    .transform_point3(d * vec3(0.0, h, 1.0))
                    .into(),
                world_from_image
                    .transform_point3(d * vec3(w, h, 1.0))
                    .into(),
                world_from_image
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
                instance_id,
                segments,
                radius: line_radius,
                color,
            });
        }
    }

    #[cfg(feature = "glow")]
    fn add_arrow(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        instance_id: InstanceIdHash,
        color: [u8; 4],
        width_scale: Option<f32>,
        _label: Option<&str>,
        arrow: &re_log_types::Arrow3D,
    ) {
        let re_log_types::Arrow3D { origin, vector } = arrow;

        let (cylinder_id, cylinder_mesh) = ctx.cache.cpu_mesh.cylinder();
        let (cone_id, cone_mesh) = ctx.cache.cpu_mesh.cone();

        let vector = glam::Vec3::from_slice(vector);
        let rotation = glam::Quat::from_rotation_arc(glam::Vec3::X, vector.normalize());
        let origin = glam::Vec3::from_slice(origin);

        let width_scale = width_scale.unwrap_or(1.0);
        let tip_length = 2.0 * width_scale;

        let cylinder_transform = glam::Affine3A::from_scale_rotation_translation(
            vec3(
                vector.length() - tip_length,
                0.5 * width_scale,
                0.5 * width_scale,
            ),
            rotation,
            origin,
        );

        self.meshes.push(MeshSource {
            instance_id,
            mesh_id: cylinder_id,
            world_from_mesh: cylinder_transform,
            cpu_mesh: cylinder_mesh,
            tint: Some(color),
        });

        // The cone has it's origin at the base, so we translate it by [-1,0,0] so the tip lines up with vector.
        let cone_transform = glam::Affine3A::from_scale_rotation_translation(
            vec3(tip_length, 1.0 * width_scale, 1.0 * width_scale),
            rotation,
            origin + vector,
        ) * glam::Affine3A::from_translation(-glam::Vec3::X);

        self.meshes.push(MeshSource {
            instance_id,
            mesh_id: cone_id,
            world_from_mesh: cone_transform,
            cpu_mesh: cone_mesh,
            tint: Some(color),
        });
    }

    fn add_box(
        &mut self,
        instance_id: InstanceIdHash,
        color: [u8; 4],
        line_radius: f32,
        label: Option<&str>,
        box3: &re_log_types::Box3,
    ) {
        let re_log_types::Box3 {
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

        if let Some(label) = label {
            self.labels.push(Label {
                text: (*label).to_owned(),
                origin: translation,
            });
        }

        self.line_segments.push(LineSegments {
            instance_id,
            segments,
            radius: line_radius,
            color,
        });
    }

    pub fn picking(
        &self,
        pointer_in_ui: egui::Pos2,
        rect: &egui::Rect,
        eye: &Eye,
    ) -> Option<(InstanceIdHash, glam::Vec3)> {
        crate::profile_function!();

        let ui_from_world = eye.ui_from_world(rect);
        let world_from_ui = eye.world_from_ui(rect);

        let ray_in_world = {
            let ray_dir =
                world_from_ui.project_point3(Vec3::new(pointer_in_ui.x, pointer_in_ui.y, -1.0))
                    - eye.pos_in_world();
            macaw::Ray3::from_origin_dir(eye.pos_in_world(), ray_dir.normalize())
        };

        let Self {
            points,
            line_segments,
            meshes,
            labels: _,
            point_radius_from_distance: _,
            line_radius_from_distance: _,
        } = self;

        // in points
        let max_side_dist_sq = 5.0 * 5.0; // TODO(emilk): interaction radius from egui

        let mut closest_z = f32::INFINITY;
        // in points
        let mut closest_side_dist_sq = max_side_dist_sq;
        let mut closest_instance_id = None;

        {
            crate::profile_scope!("points");
            for point in points {
                if point.instance_id.is_some() {
                    // TODO(emilk): take point radius into account
                    let pos_in_ui = ui_from_world.project_point3(point.pos.into());
                    if pos_in_ui.z < 0.0 {
                        continue; // TODO(emilk): don't we expect negative Z!? RHS etc
                    }
                    let dist_sq = egui::pos2(pos_in_ui.x, pos_in_ui.y).distance_sq(pointer_in_ui);
                    if dist_sq < max_side_dist_sq {
                        let t = pos_in_ui.z.abs();
                        if t < closest_z || dist_sq < closest_side_dist_sq {
                            closest_z = t;
                            closest_side_dist_sq = dist_sq;
                            closest_instance_id = Some(point.instance_id);
                        }
                    }
                }
            }
        }

        {
            crate::profile_scope!("line_segments");
            for line_segments in line_segments {
                if line_segments.instance_id.is_some() {
                    // TODO(emilk): take line segment radius into account
                    use egui::pos2;

                    for [a, b] in &line_segments.segments {
                        let a = ui_from_world.project_point3((*a).into());
                        let b = ui_from_world.project_point3((*b).into());
                        let dist_sq = line_segment_distance_sq_to_point_2d(
                            [pos2(a.x, a.y), pos2(b.x, b.y)],
                            pointer_in_ui,
                        );

                        if dist_sq < max_side_dist_sq {
                            let t = a.z.abs(); // not very accurate
                            if t < closest_z || dist_sq < closest_side_dist_sq {
                                closest_z = t;
                                closest_side_dist_sq = dist_sq;
                                closest_instance_id = Some(line_segments.instance_id);
                            }
                        }
                    }
                }
            }
        }

        {
            crate::profile_scope!("meshes");
            for mesh in meshes {
                if mesh.instance_id.is_some() {
                    let ray_in_mesh = (mesh.world_from_mesh.inverse() * ray_in_world).normalize();
                    let t = crate::math::ray_bbox_intersect(&ray_in_mesh, mesh.cpu_mesh.bbox());

                    if t < f32::INFINITY {
                        let dist_sq = 0.0;
                        if t < closest_z || dist_sq < closest_side_dist_sq {
                            closest_z = t; // TODO(emilk): I think this is wrong
                            closest_side_dist_sq = dist_sq;
                            closest_instance_id = Some(mesh.instance_id);
                        }
                    }
                }
            }
        }

        if let Some(closest_instance_id) = closest_instance_id {
            let closest_point = world_from_ui.project_point3(Vec3::new(
                pointer_in_ui.x,
                pointer_in_ui.y,
                closest_z,
            ));
            Some((closest_instance_id, closest_point))
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
