use std::sync::Arc;

use ahash::HashMap;
use egui::NumExt as _;
use glam::{vec3, Vec3};
use re_data_store::{InstanceIdHash, ObjPath, ObjectsProperties};
use re_log_types::{
    field_types::{ClassId, KeypointId},
    IndexHash, MeshId, Tensor,
};
use re_renderer::{Color32, Size};

use super::{eye::Eye, SpaceCamera3D, SpatialNavigationMode};
use crate::{
    misc::{mesh_loader::LoadedMesh, ViewerContext},
    ui::{
        annotations::{auto_color, AnnotationMap},
        transform_cache::TransformCache,
        Annotations, SceneQuery,
    },
};

mod picking;
mod primitives;
mod scene_part;

pub use self::picking::{AdditionalPickingInfo, PickingRayHit, PickingResult};
pub use self::primitives::SceneSpatialPrimitives;
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
    /// The shape being labeled.
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

    /// Picking any any of these rects cause the referred instance to be hovered.
    /// Only use this for 2d overlays!
    pub pickable_ui_rects: Vec<(egui::Rect, InstanceIdHash)>,

    /// Images are a special case of rects where we're storing some extra information to allow miniature previews etc.
    pub images: Vec<Image>,
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

pub type Keypoints = HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec3>>;

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

        //TODO(john) implement this for Arrow data store
        self.annotation_map.load(ctx, query);

        let parts: Vec<&dyn ScenePart> = vec![
            &scene_part::Points3DPartClassic,
            &scene_part::Points3DPart { max_labels: 10 },
            // --
            &scene_part::Points2DPart,
            &scene_part::Boxes3DPart,
            &scene_part::Lines3DPart,
            &scene_part::Arrows3DPart,
            &scene_part::MeshPart,
            &scene_part::ImagesPart,
            // --
            &scene_part::Boxes2DPartClassic,
            &scene_part::Boxes2DPart,
            // --
            &scene_part::LineSegments2DPart,
            &scene_part::Points2DPart,
        ];

        for part in parts {
            part.load(self, ctx, query, transforms, objects_properties, hovered);
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
        keypoints: Keypoints,
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
        let point_size_at_one_meter = eye.fov_y.unwrap() / viewport_size.y;

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
                self.primitives.add_axis_lines(
                    camera.world_from_cam(),
                    instance_id,
                    eye,
                    viewport_size,
                );
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
        ui_rect: &egui::Rect,
        eye: &Eye,
        ui_interaction_radius: f32,
    ) -> PickingResult {
        picking::picking(
            pointer_in_ui,
            ui_rect,
            eye,
            &self.primitives,
            &self.ui,
            ui_interaction_radius,
        )
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
