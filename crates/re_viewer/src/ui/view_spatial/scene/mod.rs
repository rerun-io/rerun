use std::sync::Arc;

use ahash::HashMap;
use egui::NumExt as _;
use glam::{vec3, Vec3};
use itertools::Itertools as _;

use re_data_store::{InstanceIdHash, ObjPath, ObjectsProperties};
use re_log_types::{
    context::{ClassId, KeypointId},
    IndexHash, MeshId, Tensor,
};
use re_renderer::{
    renderer::MeshInstance, Color32, LineStripSeriesBuilder, PointCloudBuilder, Size,
};

use crate::{
    math::line_segment_distance_sq_to_point_2d,
    misc::{mesh_loader::LoadedMesh, ViewerContext},
    ui::{
        annotations::{auto_color, AnnotationMap},
        transform_cache::TransformCache,
        view_spatial::axis_color,
        Annotations, SceneQuery,
    },
};

use super::{eye::Eye, SpaceCamera3D, SpatialNavigationMode};

mod scene_part;
use scene_part::ScenePart;

// ----------------------------------------------------------------------------

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

/// TODO(andreas): Scene should only care about converted rendering primitive.
pub struct MeshSource {
    pub instance_hash: InstanceIdHash,
    // TODO(andreas): Make this Conformal3 once glow is gone?
    pub world_from_mesh: macaw::Affine3A,
    pub mesh: Arc<LoadedMesh>,
    pub additive_tint: Option<Color32>,
}

pub struct Image {
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

// TODO(andreas): Merge Label2D and Label3D
pub struct Label2D {
    pub text: String,
    pub color: Color32,
    /// The shape being labled.
    pub target: Label2DTarget,
    /// What is hovered if this label is hovered.
    pub labled_instance: InstanceIdHash,
}

pub struct Label3D {
    pub(crate) text: String,
    /// Origin of the label
    pub(crate) origin: Vec3,
}

fn to_ecolor([r, g, b, a]: [u8; 4]) -> Color32 {
    // TODO(andreas): ecolor should have a utility to get an array
    Color32::from_rgba_premultiplied(r, g, b, a)
}

/// Data necessary to setup the ui [`SceneSpatial`] but of no interest to `re_renderer`.
#[derive(Default)]
pub struct SceneSpatialUiData {
    pub labels_3d: Vec<Label3D>,
    pub labels_2d: Vec<Label2D>,

    /// Cursor within any of these rects cause the referred instance to be hovered.
    pub rects: Vec<(egui::Rect, InstanceIdHash)>,

    /// Images are a special case of rects where we're storing some extra information to allow miniature previews etc.
    pub images: Vec<Image>,
}

/// Primitives sent off to `re_renderer`.
/// (Some meta information still relevant to ui setup as well)
#[derive(Default)]
pub struct SceneSpatialPrimitives {
    /// Estimated bounding box of all data in scene coordinates. Accumulated.
    bounding_box: macaw::BoundingBox,

    /// TODO(andreas): Need to decide of this should be used for hovering as well. If so add another builder with meta-data?
    pub textured_rectangles: Vec<re_renderer::renderer::TexturedRect>,
    pub line_strips: LineStripSeriesBuilder<InstanceIdHash>,
    pub points: PointCloudBuilder<InstanceIdHash>,

    pub meshes: Vec<MeshSource>,
}

impl SceneSpatialPrimitives {
    /// bounding box covering the rendered scene
    pub fn bounding_box(&self) -> macaw::BoundingBox {
        self.bounding_box
    }

    pub fn recalculate_bounding_box(&mut self) {
        crate::profile_function!();

        self.bounding_box = macaw::BoundingBox::nothing();

        for rect in &self.textured_rectangles {
            self.bounding_box.extend(rect.top_left_corner_position);
            self.bounding_box
                .extend(rect.top_left_corner_position + rect.extent_u);
            self.bounding_box
                .extend(rect.top_left_corner_position + rect.extent_v);
            self.bounding_box
                .extend(rect.top_left_corner_position + rect.extent_v + rect.extent_u);
        }

        // We don't need a very accurate bounding box, so in order to save some time,
        // we calculate a per batch bounding box for lines and points.
        // TODO(andreas): We should keep these around to speed up picking!
        for (batch, vertex_iter) in self.points.iter_vertices_by_batch() {
            let batch_bb = macaw::BoundingBox::from_points(vertex_iter.map(|v| v.position));
            self.bounding_box = self.bounding_box.union(
                batch_bb.transform_affine3(&glam::Affine3A::from_mat4(batch.world_from_obj)),
            );
        }
        for (batch, vertex_iter) in self.line_strips.iter_vertices_by_batch() {
            let batch_bb = macaw::BoundingBox::from_points(vertex_iter.map(|v| v.position));
            self.bounding_box = self.bounding_box.union(
                batch_bb.transform_affine3(&glam::Affine3A::from_mat4(batch.world_from_obj)),
            );
        }

        for mesh in &self.meshes {
            self.bounding_box = self
                .bounding_box
                .union(mesh.mesh.bbox().transform_affine3(&mesh.world_from_mesh));
        }
    }

    pub fn mesh_instances(&self) -> Vec<MeshInstance> {
        crate::profile_function!();
        self.meshes
            .iter()
            .flat_map(|mesh| {
                let (scale, rotation, translation) =
                    mesh.world_from_mesh.to_scale_rotation_translation();
                // TODO(andreas): The renderer should make it easy to apply a transform to a bunch of meshes
                let base_transform =
                    glam::Affine3A::from_scale_rotation_translation(scale, rotation, translation);
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
}

#[derive(Default)]
pub struct SceneSpatial {
    pub annotation_map: AnnotationMap,
    pub primitives: SceneSpatialPrimitives,
    pub ui: SceneSpatialUiData,
}

fn instance_hash_if_interactive(
    obj_path: &ObjPath,
    instance_index: Option<&IndexHash>,
    interactive: bool,
) -> InstanceIdHash {
    if interactive {
        InstanceIdHash::from_path_and_index(
            obj_path,
            instance_index.copied().unwrap_or(IndexHash::NONE),
        )
    } else {
        InstanceIdHash::NONE
    }
}

impl SceneSpatial {
    /// Loads all 3D objects into the scene according to the given query.
    pub(crate) fn load_objects(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        objects_properties: &ObjectsProperties,
        hovered: InstanceIdHash,
    ) {
        crate::profile_function!();

        self.annotation_map.load(ctx, query);

        let parts = [
            scene_part::Points3DPart::load,
            scene_part::Boxes3DPart::load,
            scene_part::Lines3DPart::load,
            scene_part::Arrows3DPart::load,
            scene_part::MeshPart::load,
            scene_part::ImagesPart::load,
            scene_part::Boxes2DPart::load,
            scene_part::LineSegments2DPart::load,
            scene_part::Points2DPart::load,
        ];

        for load in parts {
            (load)(self, ctx, query, transforms, objects_properties, hovered);
        }

        self.primitives.recalculate_bounding_box();
    }

    const HOVER_COLOR: Color32 = Color32::from_rgb(255, 200, 200);

    fn hover_size_boost(size: Size) -> Size {
        if size.is_auto() {
            Size::AUTO_LARGE
        } else {
            size * 1.5
        }
    }

    fn load_keypoint_connections(
        &mut self,
        obj_path: &re_data_store::ObjPath,
        keypoints: HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec3>>,
        annotations: &Arc<Annotations>,
        interactive: bool,
    ) {
        // Generate keypoint connections if any.
        let instance_hash = instance_hash_if_interactive(obj_path, None, interactive);

        let mut line_batch = self.primitives.line_strips.batch("keypoint connections");

        for ((class_id, _time), keypoints_in_class) in keypoints {
            let Some(class_description) = annotations.context.class_map.get(&class_id) else {
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
                line_batch
                    .add_segment(*a, *b)
                    .radius(Size::AUTO)
                    .color(to_ecolor(color))
                    .user_data(instance_hash);
            }
        }
    }

    // ---

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn add_cameras(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        scene_bbox: &macaw::BoundingBox,
        viewport_size: egui::Vec2,
        eye: &Eye,
        cameras: &[SpaceCamera3D],
        hovered_instance: InstanceIdHash,
        obj_properties: &ObjectsProperties,
    ) {
        crate::profile_function!();

        // Size of a pixel (in meters), when projected out one meter:
        let point_size_at_one_meter = eye.fov_y / viewport_size.y;

        let eye_camera_plane =
            macaw::Plane3::from_normal_point(eye.forward_in_world(), eye.pos_in_world());

        for camera in cameras {
            let instance_id = InstanceIdHash::from_path_and_index(
                &camera.camera_obj_path,
                camera.instance_index_hash,
            );
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

                    if let Some(cpu_mesh) = ctx.cache.mesh.load(
                        "camera_mesh",
                        &MeshSourceData::StaticGlb(
                            mesh_id,
                            include_bytes!("../../../../data/camera.glb"),
                        ),
                        ctx.render_ctx,
                    ) {
                        let additive_tint = is_hovered.then_some(Self::HOVER_COLOR);

                        self.primitives.meshes.push(MeshSource {
                            instance_hash: instance_id,
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

                let mut batch = self.primitives.line_strips.batch("camera axis");

                for (axis_index, dir) in [Vec3::X, Vec3::Y, Vec3::Z].iter().enumerate() {
                    let axis_end = world_from_cam.transform_point3(scale * *dir);
                    let color = axis_color(axis_index);

                    batch
                        .add_segment(cam_origin, axis_end)
                        .radius(Size::new_points(2.0))
                        .color(color)
                        .user_data(instance_id);
                }
            }

            let mut frustum_length = scene_bbox.size().length() * 0.3;
            if let (Some(pinhole), Some(child_space)) = (&camera.pinhole, &camera.target_space) {
                frustum_length = obj_properties
                    .get(child_space)
                    .pinhole_image_plane_distance(pinhole);
            }

            self.add_camera_frustum(camera, instance_id, line_radius, frustum_length, line_color);
        }
    }

    /// Paint frustum lines
    fn add_camera_frustum(
        &mut self,
        camera: &SpaceCamera3D,
        instance_id: InstanceIdHash,
        line_radius: Size,
        frustum_length: f32,
        color: Color32,
    ) -> Option<()> {
        let world_from_image = camera.world_from_image()?;
        let [w, h] = camera.pinhole?.resolution?;

        // TODO(emilk): there is probably a off-by-one or off-by-half error here.
        // The image coordinates are in [0, w-1] range, so either we should use those limits
        // or [-0.5, w-0.5] for the "pixels are tiny squares" interpretation of the frustum.

        let corners = [
            world_from_image.transform_point3(frustum_length * vec3(0.0, 0.0, 1.0)),
            world_from_image.transform_point3(frustum_length * vec3(0.0, h, 1.0)),
            world_from_image.transform_point3(frustum_length * vec3(w, h, 1.0)),
            world_from_image.transform_point3(frustum_length * vec3(w, 0.0, 1.0)),
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

        self.primitives
            .line_strips
            .batch("camera frustum")
            .add_segments(segments.into_iter())
            .radius(line_radius)
            .color(color)
            .user_data(instance_id);

        Some(())
    }

    /// Heuristic whether the default way of looking at this scene should be 2d or 3d.
    pub fn prefer_2d_mode(&self) -> bool {
        // If any 2D interactable picture is there we regard it as 2d.
        if !self.ui.images.is_empty() {
            return true;
        }

        // Instead a mesh indicates 3d.
        if !self.primitives.meshes.is_empty() {
            return false;
        }

        // Otherwise do an heuristic based on the z extent of bounding box
        let bbox = self.primitives.bounding_box();
        bbox.min.z >= self.primitives.line_strips.next_2d_z * 2.0 && bbox.max.z < 1.0
    }

    pub fn preferred_navigation_mode(&self) -> SpatialNavigationMode {
        if self.prefer_2d_mode() {
            SpatialNavigationMode::TwoD
        } else {
            SpatialNavigationMode::ThreeD
        }
    }

    pub fn picking(
        &self,
        pointer_in_ui: glam::Vec2,
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

        let SceneSpatialPrimitives {
            bounding_box: _,
            textured_rectangles: _, // TODO(andreas): Should be able to pick 2d rectangles!
            line_strips,
            points,
            meshes,
        } = &self.primitives;

        // in points
        let max_side_dist_sq = 5.0 * 5.0; // TODO(emilk): interaction radius from egui

        let mut closest_z = f32::INFINITY;
        // in points
        let mut closest_side_dist_sq = max_side_dist_sq;
        let mut closest_instance_id = None;

        {
            crate::profile_scope!("points_3d");

            for (batch, vertex_iter) in points.iter_vertices_and_userdata_by_batch() {
                // For getting the closest point we could transform the mouse ray into the "batch space".
                // However, we want to determine the closest point in *screen space*, meaning that we need to project all points.
                let ui_from_batch = ui_from_world * batch.world_from_obj;

                for (point, instance_hash) in vertex_iter {
                    if instance_hash.is_none() {
                        continue;
                    }

                    // TODO(emilk): take point radius into account
                    let pos_in_ui = ui_from_batch.project_point3(point.position);
                    if pos_in_ui.z < 0.0 {
                        continue; // TODO(emilk): don't we expect negative Z!? RHS etc
                    }
                    let dist_sq = pos_in_ui.truncate().distance_squared(pointer_in_ui);
                    if dist_sq < max_side_dist_sq {
                        let t = pos_in_ui.z.abs();
                        if t < closest_z || dist_sq < closest_side_dist_sq {
                            closest_z = t;
                            closest_side_dist_sq = dist_sq;
                            closest_instance_id = Some(*instance_hash);
                        }
                    }
                }
            }
        }

        {
            crate::profile_scope!("line_segments_3d");

            for (batch, vertices) in line_strips.iter_vertices_by_batch() {
                // For getting the closest point we could transform the mouse ray into the "batch space".
                // However, we want to determine the closest point in *screen space*, meaning that we need to project all points.
                let ui_from_batch = ui_from_world * batch.world_from_obj;

                for (start, end) in vertices.tuple_windows() {
                    // Skip unconnected tuples.
                    if start.strip_index != end.strip_index {
                        continue;
                    }

                    let instance_hash = line_strips.strip_user_data[start.strip_index as usize];
                    if instance_hash.is_none() {
                        continue;
                    }

                    // TODO(emilk): take line segment radius into account
                    let a = ui_from_batch.project_point3(start.position);
                    let b = ui_from_batch.project_point3(end.position);
                    let dist_sq = line_segment_distance_sq_to_point_2d(
                        [a.truncate(), b.truncate()],
                        pointer_in_ui,
                    );

                    if dist_sq < max_side_dist_sq {
                        let t = a.z.abs(); // not very accurate
                        if t < closest_z || dist_sq < closest_side_dist_sq {
                            closest_z = t;
                            closest_side_dist_sq = dist_sq;
                            closest_instance_id = Some(instance_hash);
                        }
                    }
                }
            }
        }

        {
            crate::profile_scope!("meshes");
            for mesh in meshes {
                if !mesh.instance_hash.is_some() {
                    continue;
                }
                let ray_in_mesh = (mesh.world_from_mesh.inverse() * ray_in_world).normalize();
                let t = crate::math::ray_bbox_intersect(&ray_in_mesh, mesh.mesh.bbox());

                if t < f32::INFINITY {
                    let dist_sq = 0.0;
                    if t < closest_z || dist_sq < closest_side_dist_sq {
                        closest_z = t; // TODO(emilk): I think this is wrong
                        closest_side_dist_sq = dist_sq;
                        closest_instance_id = Some(mesh.instance_hash);
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

pub struct ObjectPaintProperties {
    pub bg_stroke: egui::Stroke,
    pub fg_stroke: egui::Stroke,
}

// TODO(andreas): we're no longer using egui strokes. Replace this.
fn paint_properties(color: [u8; 4], stroke_width: Option<&f32>) -> ObjectPaintProperties {
    let bg_color = Color32::from_black_alpha(196);
    let fg_color = to_ecolor(color);
    let stroke_width = stroke_width.map_or(1.5, |w| *w);
    let bg_stroke = egui::Stroke::new(stroke_width + 2.0, bg_color);
    let fg_stroke = egui::Stroke::new(stroke_width, fg_color);

    ObjectPaintProperties {
        bg_stroke,
        fg_stroke,
    }
}

fn apply_hover_effect(paint_props: &mut ObjectPaintProperties) {
    paint_props.bg_stroke.width *= 2.0;
    paint_props.bg_stroke.color = Color32::BLACK;

    paint_props.fg_stroke.width *= 2.0;
    paint_props.fg_stroke.color = Color32::WHITE;
}
