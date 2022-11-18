use std::sync::Arc;

use egui::NumExt as _;
use glam::{vec3, Vec3};
use itertools::Itertools as _;

use re_data_store::query::{visit_type_data_1, visit_type_data_2, visit_type_data_3};
use re_data_store::{FieldName, InstanceIdHash};
use re_log_types::{DataVec, IndexHash, MeshId, MsgId, ObjectType};

use crate::misc::mesh_loader::CpuMesh;
use crate::misc::Caches;
use crate::ui::annotations::{AnnotationMap, DefaultColor};
use crate::ui::SceneQuery;
use crate::{math::line_segment_distance_sq_to_point_2d, misc::ViewerContext};

use re_renderer::renderer::*;

use super::{eye::Eye, SpaceCamera};

// ----------------------------------------------------------------------------

/// A size of something in either scene-units, screen-units, or unsized.
///
/// Implementation:
/// * If positive, this is in scene units.
/// * If negative, this is in ui points.
/// * If NaN, auto-size it.
/// Resolved in [`Scene3D::finalize_sizes_and_colors`].
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

pub struct Point3D {
    pub instance_id_hash: InstanceIdHash,
    pub pos: Vec3,
    pub radius: Size,
    pub color: [u8; 4],
}

pub struct LineSegments3D {
    pub instance_id_hash: InstanceIdHash,
    pub segments: Vec<(Vec3, Vec3)>,
    pub radius: Size,
    pub color: [u8; 4],
    pub flags: re_renderer::renderer::LineStripFlags,
}

pub enum MeshSourceData {
    Mesh3D(re_log_types::Mesh3D),

    /// e.g. the camera mesh
    StaticGlb(MeshId, &'static [u8]),
}

impl MeshSourceData {
    pub fn mesh_id(&self) -> MeshId {
        match self {
            MeshSourceData::Mesh3D(mesh) => mesh.mesh_id(),
            MeshSourceData::StaticGlb(id, _) => *id,
        }
    }
}

pub struct MeshSource {
    pub instance_id_hash: InstanceIdHash,
    // TODO(andreas): Make this Conformal3 once glow is gone?
    pub world_from_mesh: macaw::Affine3A,
    pub cpu_mesh: Arc<CpuMesh>,
    pub tint: Option<[u8; 4]>,
}

pub struct Label3D {
    pub(crate) text: String,
    /// Origin of the label
    pub(crate) origin: Vec3,
}

#[derive(Default)]
pub struct Scene3D {
    pub annotation_map: AnnotationMap,

    pub points: Vec<Point3D>,
    pub line_segments: Vec<LineSegments3D>,
    pub meshes: Vec<MeshSource>,
    pub labels: Vec<Label3D>,
}

impl Scene3D {
    /// Loads all 3D objects into the scene according to the given query.
    pub(crate) fn load_objects(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        self.annotation_map.load(ctx, query);

        self.load_points(ctx, query);
        self.load_boxes(ctx, query);
        self.load_segments(ctx, query);
        self.load_arrows(ctx, query);
        self.load_meshes(ctx, query);
    }

    fn load_points(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        query
            .iter_object_stores(ctx.log_db, &[ObjectType::Point3D])
            .for_each(|(_obj_type, obj_path, obj_store)| {
                visit_type_data_2(
                    obj_store,
                    &FieldName::from("pos"),
                    &query.time_query,
                    ("color", "radius"),
                    |instance_index: Option<&IndexHash>,
                     _time: i64,
                     _msg_id: &MsgId,
                     pos: &[f32; 3],
                     color: Option<&[u8; 4]>,
                     radius: Option<&f32>| {
                        let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                        let instance_id_hash =
                            InstanceIdHash::from_path_and_index(obj_path, instance_index);

                        let annotations = self.annotation_map.find(obj_path);
                        let color = annotations.color(
                            color,
                            None, // TODO(andreas): support class ids for points
                            obj_path,
                            DefaultColor::Random,
                        );

                        self.points.push(Point3D {
                            instance_id_hash,
                            pos: Vec3::from_slice(pos),
                            radius: radius.copied().map_or(Size::AUTO, Size::new_scene),
                            color,
                        });
                    },
                );
            });
    }

    fn load_boxes(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        for (_obj_type, obj_path, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Box3D])
        {
            visit_type_data_3(
                obj_store,
                &FieldName::from("obb"),
                &query.time_query,
                ("color", "stroke_width", "label"),
                |instance_index: Option<&IndexHash>,
                 _time: i64,
                 _msg_id: &MsgId,
                 obb: &re_log_types::Box3,
                 color: Option<&[u8; 4]>,
                 stroke_width: Option<&f32>,
                 label: Option<&String>| {
                    let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                    let line_radius = stroke_width.map_or(Size::AUTO, |w| Size::new_scene(w / 2.0));

                    let annotations = self.annotation_map.find(obj_path);
                    let color = annotations.color(
                        color,
                        None, // TODO(andreas): support class ids for boxes
                        obj_path,
                        DefaultColor::Random,
                    );
                    let label = annotations.label(label, None);

                    self.add_box(
                        InstanceIdHash::from_path_and_index(obj_path, instance_index),
                        color,
                        line_radius,
                        label,
                        obb,
                    );
                },
            );
        }
    }

    /// Both `Path3D` and `LineSegments3D`.
    fn load_segments(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let segments = query
            .iter_object_stores(
                ctx.log_db,
                &[ObjectType::Path3D, ObjectType::LineSegments3D],
            )
            .flat_map(|(obj_type, obj_path, obj_store)| {
                let mut batch = Vec::new();
                visit_type_data_2(
                    obj_store,
                    &FieldName::from("points"),
                    &query.time_query,
                    ("color", "stroke_width"),
                    |instance_index: Option<&IndexHash>,
                     _time: i64,
                     _msg_id: &MsgId,
                     points: &DataVec,
                     color: Option<&[u8; 4]>,
                     stroke_width: Option<&f32>| {
                        let what = match obj_type {
                            ObjectType::Path3D => "Path3D::points",
                            ObjectType::LineSegments3D => "LineSegments3D::points",
                            _ => return,
                        };
                        let Some(points) = points.as_vec_of_vec3(what) else { return };

                        let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                        let instance_id_hash =
                            InstanceIdHash::from_path_and_index(obj_path, instance_index);

                        let radius = stroke_width.map_or(Size::AUTO, |w| Size::new_scene(w / 2.0));

                        let annotations = self.annotation_map.find(obj_path);
                        let color = annotations.color(
                            color,
                            None, // TODO(andreas): support class ids for points
                            obj_path,
                            DefaultColor::Random,
                        );

                        let segments = points
                            .iter()
                            .tuple_windows()
                            .map(|(a, b)| (Vec3::from_slice(a), Vec3::from_slice(b)))
                            .collect();

                        batch.push(LineSegments3D {
                            instance_id_hash,
                            segments,
                            radius,
                            color,
                            flags: Default::default(),
                        });
                    },
                );
                batch
            });

        self.line_segments.extend(segments);
    }

    fn load_arrows(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        for (_obj_type, obj_path, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Arrow3D])
        {
            visit_type_data_3(
                obj_store,
                &FieldName::from("arrow3d"),
                &query.time_query,
                ("color", "width_scale", "label"),
                |instance_index: Option<&IndexHash>,
                 _time: i64,
                 _msg_id: &MsgId,
                 arrow: &re_log_types::Arrow3D,
                 color: Option<&[u8; 4]>,
                 width_scale: Option<&f32>,
                 label: Option<&String>| {
                    let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                    let instance_id_hash =
                        InstanceIdHash::from_path_and_index(obj_path, instance_index);

                    let width = width_scale.copied().unwrap_or(1.0);

                    let annotations = self.annotation_map.find(obj_path);
                    let color = annotations.color(
                        color,
                        None, // TODO(andreas): support class ids for arrows
                        obj_path,
                        DefaultColor::Random,
                    );
                    let label = annotations.label(label, None);

                    self.add_arrow(
                        ctx.cache,
                        instance_id_hash,
                        color,
                        Some(width),
                        label,
                        arrow,
                    );
                },
            );
        }
    }

    fn load_meshes(&mut self, ctx: &mut ViewerContext<'_>, query: &SceneQuery<'_>) {
        crate::profile_function!();

        let meshes = query
            .iter_object_stores(ctx.log_db, &[ObjectType::Mesh3D])
            .flat_map(|(_obj_type, obj_path, obj_store)| {
                let mut batch = Vec::new();
                visit_type_data_1(
                    obj_store,
                    &FieldName::from("mesh"),
                    &query.time_query,
                    ("color",),
                    |instance_index: Option<&IndexHash>,
                     _time: i64,
                     _msg_id: &MsgId,
                     mesh: &re_log_types::Mesh3D,
                     _color: Option<&[u8; 4]>| {
                        let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                        let Some(mesh) = ctx.cache.cpu_mesh.load(
                                &obj_path.to_string(),
                                &MeshSourceData::Mesh3D(mesh.clone()),
                                &mut ctx.render_ctx.mesh_manager,
                                &mut ctx.render_ctx.texture_manager_2d,
                            )
                            .map(|cpu_mesh| MeshSource {
                                instance_id_hash: InstanceIdHash::from_path_and_index(
                                    obj_path,
                                    instance_index,
                                ),
                                world_from_mesh: Default::default(),
                                cpu_mesh,
                                tint: None,
                            }) else { return };

                        batch.push(mesh);
                    },
                );
                batch
            });

        self.meshes.extend(meshes);
    }

    // ---

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

                    let mesh_id = MeshId(uuid::uuid!("0de12a29-64ea-40b9-898b-63686b5436af"));
                    let world_from_mesh = world_from_rub_view * glam::Affine3A::from_scale(scale);

                    if let Some(cpu_mesh) = ctx.cache.cpu_mesh.load(
                        "camera_mesh",
                        &MeshSourceData::StaticGlb(
                            mesh_id,
                            include_bytes!("../../../data/camera.glb"),
                        ),
                        &mut ctx.render_ctx.mesh_manager,
                        &mut ctx.render_ctx.texture_manager_2d,
                    ) {
                        self.meshes.push(MeshSource {
                            instance_id_hash: instance_id,
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

                    self.line_segments.push(LineSegments3D {
                        instance_id_hash: instance_id,
                        segments: vec![(cam_origin, axis_end)],
                        radius,
                        color,
                        flags: Default::default(),
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
            annotation_map: _,
            points,
            line_segments,
            meshes,
            labels: _, // always has final size. TODO(emilk): tint on hover!
        } = self;

        let hover_size_boost = 1.5;
        const HOVER_COLOR: [u8; 4] = [255, 200, 200, 255];

        let viewport_area = (viewport_size.x * viewport_size.y).at_least(1.0);

        // Size of a ui point (in meters), when projected out one meter:
        let point_size_at_one_meter = eye.fov_y / viewport_size.y.at_least(1.0);

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
                    let dist_to_eye = eye_camera_plane.distance(point.pos).at_least(0.0);
                    point.radius =
                        Size::new_scene(dist_to_eye * size_in_points * point_size_at_one_meter);
                }
                if point.instance_id_hash == hovered_instance_id_hash {
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
                        for &(start, end) in &line_segment.segments {
                            closest = closest.min(eye_camera_plane.distance(start));
                            closest = closest.min(eye_camera_plane.distance(end));
                        }
                        closest
                    } else {
                        let mut centroid = glam::DVec3::ZERO;
                        for (start, end) in &line_segment.segments {
                            centroid += start.as_dvec3();
                            centroid += end.as_dvec3();
                        }
                        let centroid =
                            centroid.as_vec3() / (2.0 * line_segment.segments.len() as f32);
                        eye_camera_plane.distance(centroid)
                    }
                    .at_least(0.0);

                    line_segment.radius =
                        Size::new_scene(dist_to_eye * size_in_points * point_size_at_one_meter);
                }
                if line_segment.instance_id_hash == hovered_instance_id_hash {
                    line_segment.radius *= hover_size_boost;
                    line_segment.color = HOVER_COLOR;
                }
            }
        }

        {
            crate::profile_scope!("meshes");
            for mesh in meshes {
                if mesh.instance_id_hash == hovered_instance_id_hash {
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
            world_from_image.transform_point3(d * vec3(0.0, 0.0, 1.0)),
            world_from_image.transform_point3(d * vec3(0.0, h, 1.0)),
            world_from_image.transform_point3(d * vec3(w, h, 1.0)),
            world_from_image.transform_point3(d * vec3(w, 0.0, 1.0)),
        ];

        let center = camera.position();

        let segments = vec![
            (center, corners[0]),     // frustum corners
            (center, corners[1]),     // frustum corners
            (center, corners[2]),     // frustum corners
            (center, corners[3]),     // frustum corners
            (corners[0], corners[1]), // `d` distance plane sides
            (corners[1], corners[2]), // `d` distance plane sides
            (corners[2], corners[3]), // `d` distance plane sides
            (corners[3], corners[0]), // `d` distance plane sides
        ];

        self.line_segments.push(LineSegments3D {
            instance_id_hash: instance_id,
            segments,
            radius: line_radius,
            color,
            flags: Default::default(),
        });

        Some(())
    }

    fn add_arrow(
        &mut self,
        _caches: &mut Caches,
        instance_id_hash: InstanceIdHash,
        color: [u8; 4],
        width_scale: Option<f32>,
        label: Option<String>,
        arrow: &re_log_types::Arrow3D,
    ) {
        drop(label); // TODO(andreas): support labels

        let re_log_types::Arrow3D { origin, vector } = arrow;

        let width_scale = width_scale.unwrap_or(1.0);
        let vector = Vec3::from_slice(vector);
        let origin = Vec3::from_slice(origin);

        let radius = width_scale * 0.5;
        let tip_length = LineStripFlags::get_triangle_cap_tip_length(radius);
        let vector_len = vector.length();
        let end = origin + vector * ((vector_len - tip_length) / vector_len);
        self.line_segments.push(LineSegments3D {
            instance_id_hash,
            segments: vec![(origin, end)],
            radius: Size::new_scene(radius),
            color,
            flags: re_renderer::renderer::LineStripFlags::CAP_END_TRIANGLE,
        });
    }

    fn add_box(
        &mut self,
        instance_id: InstanceIdHash,
        color: [u8; 4],
        line_radius: Size,
        label: Option<String>,
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
            transform.transform_point3(vec3(-0.5, -0.5, -0.5)),
            transform.transform_point3(vec3(-0.5, -0.5, 0.5)),
            transform.transform_point3(vec3(-0.5, 0.5, -0.5)),
            transform.transform_point3(vec3(-0.5, 0.5, 0.5)),
            transform.transform_point3(vec3(0.5, -0.5, -0.5)),
            transform.transform_point3(vec3(0.5, -0.5, 0.5)),
            transform.transform_point3(vec3(0.5, 0.5, -0.5)),
            transform.transform_point3(vec3(0.5, 0.5, 0.5)),
        ];

        let segments = vec![
            // bottom:
            (corners[0b000], corners[0b001]),
            (corners[0b000], corners[0b010]),
            (corners[0b011], corners[0b001]),
            (corners[0b011], corners[0b010]),
            // top:
            (corners[0b100], corners[0b101]),
            (corners[0b100], corners[0b110]),
            (corners[0b111], corners[0b101]),
            (corners[0b111], corners[0b110]),
            // sides:
            (corners[0b000], corners[0b100]),
            (corners[0b001], corners[0b101]),
            (corners[0b010], corners[0b110]),
            (corners[0b011], corners[0b111]),
        ];

        if let Some(label) = label {
            self.labels.push(Label3D {
                text: label,
                origin: translation,
            });
        }

        self.line_segments.push(LineSegments3D {
            instance_id_hash: instance_id,
            segments,
            radius: line_radius,
            color,
            flags: Default::default(),
        });
    }

    pub fn is_empty(&self) -> bool {
        let Self {
            annotation_map: _,
            points,
            line_segments,
            meshes,
            labels,
        } = self;

        points.is_empty() && line_segments.is_empty() && meshes.is_empty() && labels.is_empty()
    }

    // TODO(cmc): maybe we just store that from the beginning once glow is gone?
    pub fn line_strips(&self, show_origin_axis: bool) -> Vec<LineStrip> {
        crate::profile_function!();
        let mut line_strips = Vec::with_capacity(self.line_segments.len());
        for segments in &self.line_segments {
            let mut current_strip = LineStrip {
                points: Vec::new(),
                radius: segments.radius.0,
                color: segments.color,
                flags: segments.flags,
            };
            for &(start, end) in &segments.segments {
                if let Some(prev) = current_strip.points.last() {
                    if *prev == start {
                        current_strip.points.push(end);
                    } else {
                        line_strips.push(std::mem::replace(
                            &mut current_strip,
                            LineStrip {
                                points: vec![start, end],
                                radius: segments.radius.0,
                                color: segments.color,
                                flags: segments.flags,
                            },
                        ));
                    }
                } else {
                    current_strip.points.push(start);
                    current_strip.points.push(end);
                }
            }

            if current_strip.points.len() > 1 {
                line_strips.push(current_strip);
            }
        }

        if show_origin_axis {
            line_strips.push(LineStrip {
                points: vec![glam::Vec3::ZERO, glam::Vec3::X],
                radius: 0.01,
                color: [255, 0, 0, 255],
                flags: re_renderer::renderer::LineStripFlags::CAP_END_TRIANGLE,
            });
            line_strips.push(LineStrip {
                points: vec![glam::Vec3::ZERO, glam::Vec3::Y],
                radius: 0.01,
                color: [0, 255, 0, 255],
                flags: re_renderer::renderer::LineStripFlags::CAP_END_TRIANGLE,
            });
            line_strips.push(LineStrip {
                points: vec![glam::Vec3::ZERO, glam::Vec3::Z],
                radius: 0.01,
                color: [0, 0, 255, 255],
                flags: re_renderer::renderer::LineStripFlags::CAP_END_TRIANGLE,
            });
        }

        line_strips
    }

    pub fn meshes(&self) -> Vec<MeshInstance> {
        crate::profile_function!();
        self.meshes
            .iter()
            .flat_map(|mesh| {
                let (scale, rotation, translation) =
                    mesh.world_from_mesh.to_scale_rotation_translation();
                // TODO(andreas): The renderer should make it easy to apply a transform to a bunch of meshes
                let base_transform = macaw::Conformal3::from_scale_rotation_translation(
                    re_renderer::importer::to_uniform_scale(scale),
                    rotation,
                    translation,
                );
                mesh.cpu_mesh
                    .mesh_instances
                    .iter()
                    .map(move |instance| MeshInstance {
                        mesh: instance.mesh,
                        world_from_mesh: base_transform * instance.world_from_mesh,
                        additive_tint_srgb: mesh.tint.unwrap_or([0, 0, 0, 0]),
                    })
            })
            .collect()
    }

    // TODO(cmc): maybe we just store that from the beginning once glow is gone?
    pub fn point_cloud_points(&self) -> Vec<PointCloudPoint> {
        crate::profile_function!();
        self.points
            .iter()
            .map(|point| PointCloudPoint {
                position: point.pos,
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
            annotation_map: _,
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
                if point.instance_id_hash.is_some() {
                    // TODO(emilk): take point radius into account
                    let pos_in_ui = ui_from_world.project_point3(point.pos);
                    if pos_in_ui.z < 0.0 {
                        continue; // TODO(emilk): don't we expect negative Z!? RHS etc
                    }
                    let dist_sq = egui::pos2(pos_in_ui.x, pos_in_ui.y).distance_sq(pointer_in_ui);
                    if dist_sq < max_side_dist_sq {
                        let t = pos_in_ui.z.abs();
                        if t < closest_z || dist_sq < closest_side_dist_sq {
                            closest_z = t;
                            closest_side_dist_sq = dist_sq;
                            closest_instance_id = Some(point.instance_id_hash);
                        }
                    }
                }
            }
        }

        {
            crate::profile_scope!("line_segments");
            for line_segments in line_segments {
                if line_segments.instance_id_hash.is_some() {
                    // TODO(emilk): take line segment radius into account
                    use egui::pos2;

                    for &(start, end) in &line_segments.segments {
                        let a = ui_from_world.project_point3(start);
                        let b = ui_from_world.project_point3(end);
                        let dist_sq = line_segment_distance_sq_to_point_2d(
                            [pos2(a.x, a.y), pos2(b.x, b.y)],
                            pointer_in_ui,
                        );

                        if dist_sq < max_side_dist_sq {
                            let t = a.z.abs(); // not very accurate
                            if t < closest_z || dist_sq < closest_side_dist_sq {
                                closest_z = t;
                                closest_side_dist_sq = dist_sq;
                                closest_instance_id = Some(line_segments.instance_id_hash);
                            }
                        }
                    }
                }
            }
        }

        {
            crate::profile_scope!("meshes");
            for mesh in meshes {
                if mesh.instance_id_hash.is_some() {
                    let ray_in_mesh = (mesh.world_from_mesh.inverse() * ray_in_world).normalize();
                    let t = crate::math::ray_bbox_intersect(&ray_in_mesh, mesh.cpu_mesh.bbox());

                    if t < f32::INFINITY {
                        let dist_sq = 0.0;
                        if t < closest_z || dist_sq < closest_side_dist_sq {
                            closest_z = t; // TODO(emilk): I think this is wrong
                            closest_side_dist_sq = dist_sq;
                            closest_instance_id = Some(mesh.instance_id_hash);
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
            annotation_map: _,
            points,
            line_segments,
            meshes,
            labels,
        } = self;

        for point in points {
            bbox.extend(point.pos);
        }

        for line_segments in line_segments {
            for &(start, end) in &line_segments.segments {
                bbox.extend(start);
                bbox.extend(end);
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
