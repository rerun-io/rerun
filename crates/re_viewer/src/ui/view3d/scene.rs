use std::sync::Arc;

use egui::util::hash;
use egui::NumExt as _;
use glam::{vec3, Vec3};
use itertools::Itertools as _;

use re_data_store::InstanceIdHash;

use crate::misc::mesh_loader::CpuMesh;
use crate::{math::line_segment_distance_sq_to_point_2d, misc::ViewerContext};

#[cfg(feature = "wgpu")]
use re_renderer::renderer::*;

use super::{eye::Eye, SpaceCamera};

// ----------------------------------------------------------------------------

/// A size of something in either scene-units, screen-units, or unsized.
///
/// Implementation:
/// * If positive, this is in scene units.
/// * If negative, this is in ui points.
/// * If NaN, auto-size it.
/// Resolved in [`Scene::finalize_sizes_and_colors`].
#[derive(Clone, Copy, Debug)]
pub struct Size(pub f32);

impl Size {
    /// Automatically sized based on how many there are in the scene etc.
    const AUTO: Self = Self(f32::NAN);

    #[inline]
    pub fn new_scene(size: f32) -> Self {
        debug_assert!(size.is_finite() && size >= 0.0, "Bad size: {size}");
        Self(size)
    }

    #[inline]
    pub fn new_ui(size: f32) -> Self {
        debug_assert!(size.is_finite() && size >= 0.0, "Bad size: {size}");
        Self(-size)
    }

    #[inline]
    pub fn is_auto(&self) -> bool {
        self.0.is_nan()
    }

    /// Get the scene-size of this, if stored as a scene size.
    #[inline]
    #[allow(unused)] // wgpu is not yet using this
    pub fn scene(&self) -> Option<f32> {
        (self.0.is_finite() && self.0 >= 0.0).then_some(self.0)
    }

    /// Get the ui-size of this, if stored as a ui size.
    #[inline]
    pub fn ui(&self) -> Option<f32> {
        (self.0.is_finite() && self.0 <= 0.0).then_some(-self.0)
    }
}

impl PartialEq for Size {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0.is_nan() && other.0.is_nan() || self.0 == other.0
    }
}

impl std::ops::Mul<f32> for Size {
    type Output = Size;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        debug_assert!(rhs.is_finite() && rhs >= 0.0);
        Self(self.0 * rhs)
    }
}

impl std::ops::MulAssign<f32> for Size {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        debug_assert!(rhs.is_finite() && rhs >= 0.0);
        self.0 *= rhs;
    }
}

// ----------------------------------------------------------------------------

pub struct Point {
    pub instance_id: InstanceIdHash,
    pub pos: [f32; 3],
    pub radius: Size,
    pub color: [u8; 4],
}

pub struct LineSegments {
    pub instance_id: InstanceIdHash,
    pub segments: Vec<[[f32; 3]; 2]>,
    pub radius: Size,
    pub color: [u8; 4],
}

pub enum MeshSourceData {
    Mesh3D(re_log_types::Mesh3D),
    /// e.g. the camera mesh
    StaticGlb(&'static [u8]),
}

pub struct MeshSource {
    pub instance_id: InstanceIdHash,
    pub mesh_id: u64,
    // TODO(andreas): Make this Conformal3 once glow is gone
    pub world_from_mesh: macaw::Affine3A,
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
}

impl Scene {
    pub(crate) fn from_objects(
        ctx: &mut ViewerContext<'_>,
        objects: &re_data_store::Objects<'_>,
    ) -> Self {
        crate::profile_function!();

        // hack because three-d handles colors wrong. TODO(emilk): fix three-d
        let gamma_lut = (0..=255)
            .map(|c| ((c as f32 / 255.0).powf(2.2) * 255.0).round() as u8)
            .collect_vec();
        let gamma_lut = &gamma_lut[0..256]; // saves us bounds checks later.

        let object_color = |ctx: &mut ViewerContext<'_>,
                            props: &re_data_store::InstanceProps<'_>| {
            let [r, g, b, a] = if let Some(color) = props.color {
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

        let mut scene = Scene::default();

        {
            crate::profile_scope!("point3d");
            scene.points.reserve(objects.point3d.len());
            for (props, obj) in objects.point3d.iter() {
                let re_data_store::Point3D { pos, radius } = *obj;
                scene.points.push(Point {
                    instance_id: InstanceIdHash::from_props(props),
                    pos: *pos,
                    radius: radius.map_or(Size::AUTO, Size::new_scene),
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
                let line_radius = stroke_width.map_or(Size::AUTO, |w| Size::new_scene(w / 2.0));
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

                let radius = stroke_width.map_or(Size::AUTO, |w| Size::new_scene(w / 2.0));
                let color = object_color(ctx, props);

                let segments = points
                    .iter()
                    .tuple_windows()
                    .map(|(a, b)| [*a, *b])
                    .collect();

                scene.line_segments.push(LineSegments {
                    instance_id: InstanceIdHash::from_props(props),
                    segments,
                    radius,
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

                let radius = stroke_width.map_or(Size::AUTO, |w| Size::new_scene(w / 2.0));
                let color = object_color(ctx, props);

                scene.line_segments.push(LineSegments {
                    instance_id: InstanceIdHash::from_props(props),
                    segments: bytemuck::allocation::pod_collect_to_vec(points),
                    radius,
                    color,
                });
            }
        }

        {
            crate::profile_scope!("mesh3d");
            for (props, obj) in objects.mesh3d.iter() {
                let re_data_store::Mesh3D { mesh } = *obj;
                let mesh_id = egui::util::hash(props.msg_id);
                if let Some(cpu_mesh) = ctx.cache.cpu_mesh.load(
                    mesh_id,
                    &props.obj_path.to_string(),
                    &MeshSourceData::Mesh3D(mesh.clone()),
                ) {
                    // TODO(emilk): props.color
                    scene.meshes.push(MeshSource {
                        instance_id: InstanceIdHash::from_props(props),
                        mesh_id,
                        world_from_mesh: macaw::Affine3A::IDENTITY,
                        cpu_mesh,
                        tint: None,
                    });
                }
            }
        }

        #[cfg(feature = "glow")]
        // TODO:
        {
            crate::profile_scope!("arrow3d");
            for (props, obj) in objects.arrow3d.iter() {
                let re_data_store::Arrow3D {
                    arrow,
                    label,
                    width_scale,
                } = obj;
                let width = width_scale.unwrap_or(1.0);
                let color = object_color(ctx, props);
                let instance_id = InstanceIdHash::from_props(props);
                scene.add_arrow(ctx, instance_id, color, Some(width), *label, arrow);
            }
        }

        scene
    }

    pub(super) fn add_cameras(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        scene_bbox: &macaw::BoundingBox,
        viewport_size: egui::Vec2,
        eye: &Eye,
        cameras: &[SpaceCamera],
    ) {
        crate::profile_function!();

        let line_radius_in_points = (0.0005 * viewport_size.length()).clamp(1.5, 5.0);

        // Size of a pixel (in meters), when projected out one meter:
        let point_size_at_one_meter = eye.fov_y / viewport_size.y;

        let line_radius_from_distance = line_radius_in_points * point_size_at_one_meter;

        let eye_camera_plane =
            macaw::Plane3::from_normal_point(eye.forward_in_world(), eye.pos_in_world());

        for camera in cameras {
            let instance_id = InstanceIdHash {
                obj_path_hash: *camera.camera_obj_path.hash(),
                instance_index_hash: camera.instance_index_hash,
            };

            let dist_to_eye = eye_camera_plane.distance(camera.position()).at_least(0.0);
            let color = [255, 128, 128, 255]; // TODO(emilk): camera color

            let scale_based_on_scene_size = 0.05 * scene_bbox.size().length();
            let scale_based_on_distance = dist_to_eye * point_size_at_one_meter * 50.0; // shrink as we get very close. TODO(emilk): fade instead!
            let scale = scale_based_on_scene_size.min(scale_based_on_distance);

            if ctx.options.show_camera_mesh_in_3d {
                if let Some(world_from_rub_view) = camera.world_from_rub_view() {
                    // The camera mesh file is 1m long in RUB (X=Right, Y=Up, Z=Back).
                    // The lens is at the origin.

                    let scale = Vec3::splat(scale);

                    let mesh_id = hash("camera_mesh");
                    let world_from_mesh = world_from_rub_view * glam::Affine3A::from_scale(scale);

                    if let Some(cpu_mesh) = ctx.cache.cpu_mesh.load(
                        mesh_id,
                        "camera_mesh",
                        &MeshSourceData::StaticGlb(include_bytes!("../../../data/camera.glb")),
                    ) {
                        self.meshes.push(MeshSource {
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
                let world_from_cam = camera.world_from_cam();

                // TODO(emilk): include the names of the axes ("Right", "Down", "Forward", etc)
                let cam_origin = camera.position();
                let radius = Size::new_scene(dist_to_eye * line_radius_from_distance * 2.0);

                for (axis_index, dir) in [Vec3::X, Vec3::Y, Vec3::Z].iter().enumerate() {
                    let axis_end = world_from_cam.transform_point3(scale * *dir);
                    let color = axis_color(axis_index);

                    self.line_segments.push(LineSegments {
                        instance_id,
                        segments: vec![[cam_origin.into(), axis_end.into()]],
                        radius,
                        color,
                    });
                }
            }

            let line_radius = Size::new_scene(dist_to_eye * line_radius_from_distance);
            self.add_camera_frustum(camera, scene_bbox, instance_id, line_radius, color);
        }
    }

    // TODO(andreas): A lof of the things this method does, the renderer should be able to do for us
    /// Translate screen-space sizes (ui points) and missing sizes, into proper
    /// scene-space sizes.
    ///
    /// Also does hover-effects (changing colors and sizes)
    ///
    /// Non-finite sizes are given default sizes.
    /// Negative sizes are interpreted as ui points, and are translated
    /// to screen-space sizes (based on distance).
    pub fn finalize_sizes_and_colors(
        &mut self,
        viewport_size: egui::Vec2,
        eye: &Eye,
        hovered_instance_id_hash: InstanceIdHash,
    ) {
        crate::profile_function!();

        let Self {
            points,
            line_segments,
            meshes,
            labels: _, // always has final size. TODO(emilk): tint on hover!
        } = self;

        let hover_size_boost = 1.5;
        const HOVER_COLOR: [u8; 4] = [255, 200, 200, 255];

        let viewport_area = viewport_size.x * viewport_size.y;

        // Size of a ui point (in meters), when projected out one meter:
        let point_size_at_one_meter = eye.fov_y / viewport_size.y;

        let eye_camera_plane =
            macaw::Plane3::from_normal_point(eye.forward_in_world(), eye.pos_in_world());

        // More points -> smaller points
        let default_point_radius = Size::new_ui(
            (0.3 * (viewport_area / (points.len() + 1) as f32).sqrt()).clamp(0.1, 5.0),
        );

        // TODO(emilk): more line segments -> thinner lines
        let default_line_radius = Size::new_ui((0.0005 * viewport_size.length()).clamp(1.5, 5.0));

        {
            crate::profile_scope!("points");
            for point in points {
                if point.radius.is_auto() {
                    point.radius = default_point_radius;
                }
                if let Some(size_in_points) = point.radius.ui() {
                    let dist_to_eye = eye_camera_plane
                        .distance(Vec3::from(point.pos))
                        .at_least(0.0);
                    point.radius =
                        Size::new_scene(dist_to_eye * size_in_points * point_size_at_one_meter);
                }
                if point.instance_id == hovered_instance_id_hash {
                    point.radius *= hover_size_boost;
                    point.color = HOVER_COLOR;
                }
            }
        }

        {
            crate::profile_scope!("lines");
            for line_segment in line_segments {
                if line_segment.radius.is_auto() {
                    line_segment.radius = default_line_radius;
                }
                if let Some(size_in_points) = line_segment.radius.ui() {
                    let dist_to_eye = if true {
                        // This works much better when one line segment is very close to the camera
                        let mut closest = f32::INFINITY;
                        for segment in &line_segment.segments {
                            for &endpoint in segment {
                                closest = closest.min(eye_camera_plane.distance(endpoint.into()));
                            }
                        }
                        closest
                    } else {
                        let mut centroid = glam::DVec3::ZERO;
                        for segment in &line_segment.segments {
                            centroid += glam::Vec3::from(segment[0]).as_dvec3();
                            centroid += glam::Vec3::from(segment[1]).as_dvec3();
                        }
                        let centroid =
                            centroid.as_vec3() / (2.0 * line_segment.segments.len() as f32);
                        eye_camera_plane.distance(centroid)
                    }
                    .at_least(0.0);

                    line_segment.radius =
                        Size::new_scene(dist_to_eye * size_in_points * point_size_at_one_meter);
                }
                if line_segment.instance_id == hovered_instance_id_hash {
                    line_segment.radius *= hover_size_boost;
                    line_segment.color = HOVER_COLOR;
                }
            }
        }

        {
            crate::profile_scope!("meshes");
            for mesh in meshes {
                if mesh.instance_id == hovered_instance_id_hash {
                    mesh.tint = Some(HOVER_COLOR);
                }
            }
        }
    }

    /// Paint frustum lines
    fn add_camera_frustum(
        &mut self,
        camera: &SpaceCamera,
        scene_bbox: &macaw::BoundingBox,
        instance_id: InstanceIdHash,
        line_radius: Size,
        color: [u8; 4],
    ) -> Option<()> {
        let world_from_image = camera.world_from_image()?;
        let [w, h] = camera.pinhole?.resolution?;

        // At what distance do we end the frustum?
        let d = scene_bbox.size().length() * 0.3;

        // TODO(emilk): there is probably a off-by-one or off-by-half error here.
        // The image coordinates are in [0, w-1] range, so either we should use those limits
        // or [-0.5, w-0.5] for the "pixels are tiny squares" interpretation of the frustum.

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

        let center = camera.position().into();

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

        Some(())
    }

    #[cfg(feature = "glow")]
    // TODO:
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

        let vector = Vec3::from_slice(vector);
        let rotation = glam::Quat::from_rotation_arc(Vec3::X, vector.normalize());
        let origin = Vec3::from_slice(origin);

        let width_scale = width_scale.unwrap_or(1.0);
        let tip_length = 2.0 * width_scale;

        let cylinder_transform = macaw::Affine3A::from_scale_rotation_translation(
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
        ) * glam::Affine3A::from_translation(-Vec3::X);

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
        line_radius: Size,
        label: Option<&str>,
        box3: &re_log_types::Box3,
    ) {
        let re_log_types::Box3 {
            rotation,
            translation,
            half_size,
        } = box3;
        let rotation = glam::Quat::from_array(*rotation);
        let translation = Vec3::from(*translation);
        let half_size = Vec3::from(*half_size);
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

    #[cfg(feature = "wgpu")]
    pub fn line_strips(&self) -> Vec<LineStrip> {
        let mut line_strips = Vec::with_capacity(self.line_segments.len());
        for segments in &self.line_segments {
            let mut current_strip = LineStrip {
                points: Vec::new(),
                radius: segments.radius.0,
                color: segments.color,
            };
            for [a, b] in &segments.segments {
                let a = glam::Vec3::from(*a);
                let b = glam::Vec3::from(*b);

                if let Some(prev) = current_strip.points.last() {
                    if *prev == a {
                        current_strip.points.push(b);
                    } else {
                        line_strips.push(std::mem::replace(
                            &mut current_strip,
                            LineStrip {
                                points: vec![a, b],
                                radius: segments.radius.0,
                                color: segments.color,
                            },
                        ));
                    }
                } else {
                    current_strip.points.push(a);
                    current_strip.points.push(b);
                }
            }

            if current_strip.points.len() > 1 {
                line_strips.push(current_strip);
            }
        }
        line_strips
    }

    #[cfg(feature = "wgpu")]
    pub fn point_cloud_points(&self) -> Vec<PointCloudPoint> {
        self.points
            .iter()
            .map(|point| PointCloudPoint {
                position: glam::Vec3::from(point.pos),
                radius: point.radius.0,
                srgb_color: point.color,
            })
            .collect()
    }

    pub fn picking(
        &self,
        pointer_in_ui: egui::Pos2,
        rect: &egui::Rect,
        eye: &Eye,
    ) -> Option<(InstanceIdHash, Vec3)> {
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

    pub fn calc_bbox(&self) -> macaw::BoundingBox {
        crate::profile_function!();

        let mut bbox = macaw::BoundingBox::nothing();

        let Self {
            points,
            line_segments,
            meshes,
            labels,
        } = self;

        for point in points {
            bbox.extend(point.pos.into());
        }

        for line_segments in line_segments {
            for line_segment in &line_segments.segments {
                for &endpoint in line_segment {
                    bbox.extend(endpoint.into());
                }
            }
        }

        for mesh in meshes {
            let mesh_bbox = mesh
                .cpu_mesh
                .bbox()
                .transform_affine3(&mesh.world_from_mesh);
            bbox = bbox.union(mesh_bbox);
        }

        for label in labels {
            bbox.extend(label.origin);
        }

        bbox
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
