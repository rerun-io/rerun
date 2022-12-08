use std::sync::Arc;

use ahash::HashMap;
use egui::NumExt as _;
use glam::{vec3, Vec3};
use itertools::Itertools as _;

use re_data_store::query::{
    visit_type_data_1, visit_type_data_2, visit_type_data_3, visit_type_data_4, visit_type_data_5,
};
use re_data_store::{FieldName, InstanceIdHash};
use re_log_types::context::{ClassId, KeypointId};
use re_log_types::{DataVec, IndexHash, MeshId, MsgId, ObjectType, Tensor};

use crate::misc::mesh_loader::CpuMesh;
use crate::ui::annotations::{auto_color, AnnotationMap, DefaultColor};
use crate::ui::view_spatial::axis_color;
use crate::ui::{Annotations, SceneQuery};
use crate::{math::line_segment_distance_sq_to_point_2d, misc::ViewerContext};

use re_renderer::{
    renderer::{LineStripFlags, MeshInstance, PointCloudPoint},
    Color32, Size,
};

use super::{eye::Eye, SpaceCamera3D};

// ----------------------------------------------------------------------------

pub struct Point3D {
    pub instance_id_hash: InstanceIdHash,
    pub pos: Vec3,
    pub radius: Size,
    pub color: Color32,
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
    pub mesh: Arc<CpuMesh>,
    pub additive_tint: Option<Color32>,
}

pub struct Label3D {
    pub(crate) text: String,
    /// Origin of the label
    pub(crate) origin: Vec3,
}

pub struct HoverableImage {
    pub instance_hash: InstanceIdHash,

    pub tensor: Tensor,
    /// If this is a depth map, how long is a meter?
    ///
    /// For instance, with a `u16` dtype one might have
    /// `meter == 1000.0` for millimeter precision
    /// up to a ~65m range.
    pub meter: Option<f32>,

    /// A thing that provides additional semantic context for your dtype.
    pub annotations: Arc<Annotations>,
}

pub enum Label2DTarget {
    /// Labels a given rect (in scene coordinates)
    Rect(egui::Rect),
    /// Labels a given point (in scene coordinates)
    Point(egui::Pos2),
}

pub struct Label2D {
    pub text: String,
    pub color: Color32,
    /// The shape being labled.
    pub target: Label2DTarget,
    /// What is hovered if this label is hovered.
    pub labled_instance: InstanceIdHash,
}

fn to_ecolor([r, g, b, a]: [u8; 4]) -> Color32 {
    // TODO(andreas): ecolor should have a utility to get an array
    Color32::from_rgba_premultiplied(r, g, b, a)
}

#[derive(Default)]
pub struct Scene3D {
    pub annotation_map: AnnotationMap,

    pub points_3d: Vec<Point3D>,

    // TODO(andreas) should not distinguish 2d & 3d line strips. Currently need to since we do hover checking on them directly.
    pub line_strips_3d: re_renderer::LineStripSeriesBuilder<InstanceIdHash>,

    pub meshes: Vec<MeshSource>,
    pub labels_3d: Vec<Label3D>,
}

impl Scene3D {
    /// Loads all 3D objects into the scene according to the given query.
    pub(crate) fn load_objects(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        self.annotation_map.load(ctx, query);

        self.load_points(ctx, query, hovered_instance);
        self.load_boxes(ctx, query, hovered_instance);
        self.load_lines(ctx, query, hovered_instance);
        self.load_arrows(ctx, query, hovered_instance);
        self.load_meshes(ctx, query, hovered_instance);
    }

    const HOVER_COLOR: Color32 = Color32::from_rgb(255, 200, 200);

    fn hover_size_boost(size: Size) -> Size {
        if size.is_auto() {
            Size::AUTO_LARGE
        } else {
            size * 1.5
        }
    }

    fn load_points(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        query
            .iter_object_stores(ctx.log_db, &[ObjectType::Point3D])
            .for_each(|(_obj_type, obj_path, time_query, obj_store)| {
                let mut batch_size = 0;
                let mut show_labels = true;
                let mut label_batch = Vec::new();

                // If keypoints ids show up we may need to connect them later!
                // We include time in the key, so that the "Visible history" (time range queries) feature works.
                let mut keypoints: HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec3>> =
                    Default::default();

                let annotations = self.annotation_map.find(obj_path);
                let default_color = DefaultColor::ObjPath(obj_path);

                visit_type_data_5(
                    obj_store,
                    &FieldName::from("pos"),
                    &time_query,
                    ("color", "radius", "label", "class_id", "keypoint_id"),
                    |instance_index: Option<&IndexHash>,
                    time: i64,
                     _msg_id: &MsgId,
                     pos: &[f32; 3],
                     color: Option<&[u8; 4]>,
                     radius: Option<&f32>,
                     label: Option<&String>,
                     class_id: Option<&i32>,
                     keypoint_id: Option<&i32>| {
                        batch_size += 1;

                        let pos = Vec3::from_slice(pos);

                        let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                        let instance_id_hash =
                            InstanceIdHash::from_path_and_index(obj_path, instance_index);

                        let class_id = class_id.map(|i| ClassId(*i as _));
                        let class_description = annotations.class_description(class_id);

                        let annotation_info = if let Some(keypoint_id) = keypoint_id {
                            let keypoint_id = KeypointId(*keypoint_id as _);
                            if let Some(class_id) = class_id {
                                keypoints
                                    .entry((class_id, time))
                                    .or_insert_with(Default::default)
                                    .insert(keypoint_id, pos);
                            }

                            class_description.annotation_info_with_keypoint(keypoint_id)
                        } else {
                            class_description.annotation_info()
                        };

                        let mut color = to_ecolor(annotation_info.color(color, default_color));
                        let mut radius = radius.copied().map_or(Size::AUTO, Size::new_scene);

                        if instance_id_hash == hovered_instance {
                            color = Self::HOVER_COLOR;
                            radius = Self::hover_size_boost(radius);
                        }

                        show_labels = batch_size < 10;
                        if show_labels {
                            if let Some(label) = annotation_info.label(label) {
                                label_batch.push(Label3D {
                                    text: label,
                                    origin: pos,
                                });
                            }
                        }

                        self.points_3d.push(Point3D {
                            instance_id_hash,
                            pos,
                            radius,
                            color,
                        });
                    },
                );

                if show_labels {
                    self.labels_3d.extend(label_batch);
                }

                // Generate keypoint connections if any.
                let instance_id_hash = InstanceIdHash::from_path_and_index(obj_path, IndexHash::NONE);
                for ((class_id, _time), keypoints_in_class) in &keypoints {
                    let Some(class_description) = annotations.context.class_map.get(class_id) else {
                        continue;
                    };

                    let color = class_description
                        .info
                        .color
                        .unwrap_or_else(|| auto_color(class_description.info.id));

                    for (a, b) in &class_description.keypoint_connections {
                        let (Some(a), Some(b)) = (keypoints_in_class.get(a), keypoints_in_class.get(b)) else {
                            re_log::warn_once!(
                                "Keypoint connection from index {:?} to {:?} could not be resolved in object {:?}",
                                a, b, obj_path
                            );
                            continue;
                        };
                        self.line_strips_3d.add_segment(*a, *b).radius(Size::AUTO).color(to_ecolor(color)).user_data(instance_id_hash);
                    }
                }
            });
    }

    fn load_boxes(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Box3D])
        {
            let annotations = self.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);

            visit_type_data_4(
                obj_store,
                &FieldName::from("obb"),
                &time_query,
                ("color", "stroke_width", "label", "class_id"),
                |instance_index: Option<&IndexHash>,
                 _time: i64,
                 _msg_id: &MsgId,
                 obb: &re_log_types::Box3,
                 color: Option<&[u8; 4]>,
                 stroke_width: Option<&f32>,
                 label: Option<&String>,
                 class_id: Option<&i32>| {
                    let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);
                    let mut line_radius =
                        stroke_width.map_or(Size::AUTO, |w| Size::new_scene(w / 2.0));

                    let annotation_info = annotations
                        .class_description(class_id.map(|i| ClassId(*i as _)))
                        .annotation_info();
                    let mut color = to_ecolor(annotation_info.color(color, default_color));
                    let label = annotation_info.label(label);

                    let instance_id_hash =
                        InstanceIdHash::from_path_and_index(obj_path, instance_index);
                    if instance_id_hash == hovered_instance {
                        color = Self::HOVER_COLOR;
                        line_radius = Self::hover_size_boost(line_radius);
                    }

                    self.add_box(instance_id_hash, color, line_radius, label, obb);
                },
            );
        }
    }

    /// Both `Path3D` and `LineSegments3D`.
    fn load_lines(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        for (obj_type, obj_path, time_query, obj_store) in query.iter_object_stores(
            ctx.log_db,
            &[ObjectType::Path3D, ObjectType::LineSegments3D],
        ) {
            let annotations = self.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);

            visit_type_data_2(
                obj_store,
                &FieldName::from("points"),
                &time_query,
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

                    let mut radius = stroke_width.map_or(Size::AUTO, |w| Size::new_scene(w / 2.0));

                    // TODO(andreas): support class ids for lines
                    let annotation_info = annotations.class_description(None).annotation_info();
                    let mut color = to_ecolor(annotation_info.color(color, default_color));

                    if instance_id_hash == hovered_instance {
                        color = Self::HOVER_COLOR;
                        radius = Self::hover_size_boost(radius);
                    }

                    match obj_type {
                        ObjectType::Path3D => self
                            .line_strips_3d
                            .add_strip(points.iter().map(|v| Vec3::from_slice(v))),
                        ObjectType::LineSegments3D => self.line_strips_3d.add_segments(
                            points
                                .chunks_exact(2)
                                .map(|points| (points[0].into(), points[1].into())),
                        ),
                        _ => unreachable!("already early outed earlier"),
                    }
                    .radius(radius)
                    .color(color)
                    .user_data(instance_id_hash);
                },
            );
        }
    }

    fn load_arrows(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        for (_obj_type, obj_path, time_query, obj_store) in
            query.iter_object_stores(ctx.log_db, &[ObjectType::Arrow3D])
        {
            let annotations = self.annotation_map.find(obj_path);
            let default_color = DefaultColor::ObjPath(obj_path);

            visit_type_data_3(
                obj_store,
                &FieldName::from("arrow3d"),
                &time_query,
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

                    // TODO(andreas): support class ids for arrows
                    let annotation_info = annotations.class_description(None).annotation_info();
                    let color = annotation_info.color(color, default_color);
                    let label = annotation_info.label(label);

                    self.add_arrow(
                        instance_id_hash,
                        hovered_instance,
                        color,
                        Some(width),
                        label,
                        arrow,
                    );
                },
            );
        }
    }

    fn load_meshes(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        let meshes = query
            .iter_object_stores(ctx.log_db, &[ObjectType::Mesh3D])
            .flat_map(|(_obj_type, obj_path, time_query, obj_store)| {
                let mut batch = Vec::new();
                visit_type_data_1(
                    obj_store,
                    &FieldName::from("mesh"),
                    &time_query,
                    ("color",),
                    |instance_index: Option<&IndexHash>,
                     _time: i64,
                     _msg_id: &MsgId,
                     mesh: &re_log_types::Mesh3D,
                     _color: Option<&[u8; 4]>| {
                        let instance_index = instance_index.copied().unwrap_or(IndexHash::NONE);

                        let instance_id_hash =
                            InstanceIdHash::from_path_and_index(obj_path, instance_index);

                        let additive_tint = if hovered_instance == instance_id_hash {
                            Some(Self::HOVER_COLOR)
                        } else {
                            None
                        };

                        let Some(mesh) = ctx.cache.cpu_mesh.load(
                                &obj_path.to_string(),
                                &MeshSourceData::Mesh3D(mesh.clone()),
                                ctx.render_ctx
                            )
                            .map(|cpu_mesh| MeshSource {
                                instance_id_hash,
                                world_from_mesh: Default::default(),
                                mesh: cpu_mesh,
                                additive_tint,
                            }) else { return };

                        batch.push(mesh);
                    },
                );
                batch
            });

        self.meshes.extend(meshes);
    }

    // ---

    pub(crate) fn add_cameras(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        scene_bbox: &macaw::BoundingBox,
        viewport_size: egui::Vec2,
        eye: &Eye,
        cameras: &[SpaceCamera3D],
        hovered_instance: InstanceIdHash,
    ) {
        crate::profile_function!();

        // Size of a pixel (in meters), when projected out one meter:
        let point_size_at_one_meter = eye.fov_y / viewport_size.y;

        let eye_camera_plane =
            macaw::Plane3::from_normal_point(eye.forward_in_world(), eye.pos_in_world());

        for camera in cameras {
            let instance_id = InstanceIdHash {
                obj_path_hash: *camera.camera_obj_path.hash(),
                instance_index_hash: camera.instance_index_hash,
            };
            let is_hovered = instance_id == hovered_instance;

            let (line_radius, line_color) = if is_hovered {
                (Size::AUTO_LARGE, Self::HOVER_COLOR)
            } else {
                (Size::AUTO, Color32::from_rgb(255, 128, 128))
            }; // TODO(emilk): camera color

            let scale_based_on_scene_size = 0.05 * scene_bbox.size().length();
            let dist_to_eye = eye_camera_plane.distance(camera.position()).at_least(0.0);
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
                        ctx.render_ctx,
                    ) {
                        let additive_tint = is_hovered.then_some(Self::HOVER_COLOR);

                        self.meshes.push(MeshSource {
                            instance_id_hash: instance_id,
                            world_from_mesh,
                            mesh: cpu_mesh,
                            additive_tint,
                        });
                    }
                }
            }

            if ctx.options.show_camera_axes_in_3d {
                let world_from_cam = camera.world_from_cam();

                // TODO(emilk): include the names of the axes ("Right", "Down", "Forward", etc)
                let cam_origin = camera.position();

                for (axis_index, dir) in [Vec3::X, Vec3::Y, Vec3::Z].iter().enumerate() {
                    let axis_end = world_from_cam.transform_point3(scale * *dir);
                    let color = axis_color(axis_index);

                    self.line_strips_3d
                        .add_segment(cam_origin, axis_end)
                        .radius(Size::new_points(2.0))
                        .color(color)
                        .user_data(instance_id);
                }
            }

            self.add_camera_frustum(camera, scene_bbox, instance_id, line_radius, line_color);
        }
    }

    /// Paint frustum lines
    fn add_camera_frustum(
        &mut self,
        camera: &SpaceCamera3D,
        scene_bbox: &macaw::BoundingBox,
        instance_id: InstanceIdHash,
        line_radius: Size,
        color: Color32,
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

        let segments = [
            (center, corners[0]),     // frustum corners
            (center, corners[1]),     // frustum corners
            (center, corners[2]),     // frustum corners
            (center, corners[3]),     // frustum corners
            (corners[0], corners[1]), // `d` distance plane sides
            (corners[1], corners[2]), // `d` distance plane sides
            (corners[2], corners[3]), // `d` distance plane sides
            (corners[3], corners[0]), // `d` distance plane sides
        ];

        self.line_strips_3d
            .add_segments(segments.into_iter())
            .radius(line_radius)
            .color(color)
            .user_data(instance_id);

        Some(())
    }

    fn add_arrow(
        &mut self,
        instance_id_hash: InstanceIdHash,
        hovered_instance: InstanceIdHash,
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

        let mut radius = Size::new_scene(width_scale * 0.5);
        let tip_length = LineStripFlags::get_triangle_cap_tip_length(radius.0);
        let vector_len = vector.length();
        let end = origin + vector * ((vector_len - tip_length) / vector_len);

        let mut color = to_ecolor(color);
        if instance_id_hash == hovered_instance {
            color = Self::HOVER_COLOR;
            radius = Self::hover_size_boost(radius);
        }

        self.line_strips_3d
            .add_segment(origin, end)
            .radius(radius)
            .color(color)
            .flags(re_renderer::renderer::LineStripFlags::CAP_END_TRIANGLE)
            .user_data(instance_id_hash);
    }

    fn add_box(
        &mut self,
        instance_id: InstanceIdHash,
        color: Color32,
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

        if let Some(label) = label {
            self.labels_3d.push(Label3D {
                text: label,
                origin: translation,
            });
        }

        let segments = [
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

        self.line_strips_3d
            .add_segments(segments.into_iter())
            .radius(line_radius)
            .color(color)
            .user_data(instance_id);
    }

    pub fn is_empty(&self) -> bool {
        let Self {
            annotation_map: _,
            points_3d: points,
            line_strips_3d: line_strips,
            meshes,
            labels_3d: labels,
        } = self;

        points.is_empty() && line_strips.strips.is_empty() && meshes.is_empty() && labels.is_empty()
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
                mesh.mesh
                    .mesh_instances
                    .iter()
                    .map(move |instance| MeshInstance {
                        gpu_mesh: instance.gpu_mesh.clone(),
                        mesh: None, // Don't care.
                        world_from_mesh: base_transform * instance.world_from_mesh,
                        additive_tint: mesh.additive_tint.unwrap_or(Color32::TRANSPARENT),
                    })
            })
            .collect()
    }

    // TODO(cmc): maybe we just store that from the beginning once glow is gone?
    pub fn point_cloud_points(&self) -> Vec<PointCloudPoint> {
        crate::profile_function!();
        self.points_3d
            .iter()
            .map(|point| PointCloudPoint {
                position: point.pos,
                radius: point.radius,
                color: point.color,
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
            points_3d: points,
            line_strips_3d: line_strips,
            meshes,
            labels_3d: _,
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
            for ((_line_strip, vertices), instance_id_hash) in line_strips
                .iter_strips_with_vertices()
                .zip(line_strips.strip_user_data.iter())
            {
                if !instance_id_hash.is_some() {
                    continue;
                }
                // TODO(emilk): take line segment radius into account
                use egui::pos2;

                for (start, end) in vertices.tuple_windows() {
                    let a = ui_from_world.project_point3(start.pos);
                    let b = ui_from_world.project_point3(end.pos);
                    let dist_sq = line_segment_distance_sq_to_point_2d(
                        [pos2(a.x, a.y), pos2(b.x, b.y)],
                        pointer_in_ui,
                    );

                    if dist_sq < max_side_dist_sq {
                        let t = a.z.abs(); // not very accurate
                        if t < closest_z || dist_sq < closest_side_dist_sq {
                            closest_z = t;
                            closest_side_dist_sq = dist_sq;
                            closest_instance_id = Some(*instance_id_hash);
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
                    let t = crate::math::ray_bbox_intersect(&ray_in_mesh, mesh.mesh.bbox());

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
            points_3d: points,
            line_strips_3d: line_strips,
            meshes,
            labels_3d: labels,
        } = self;

        for point in points {
            bbox.extend(point.pos);
        }

        for vertex in &line_strips.vertices {
            bbox.extend(vertex.pos);
        }

        for mesh in meshes {
            let mesh_bbox = mesh.mesh.bbox().transform_affine3(&mesh.world_from_mesh);
            bbox = bbox.union(mesh_bbox);
        }

        for label in labels {
            bbox.extend(label.origin);
        }

        bbox
    }
}
