use std::sync::Arc;

use ahash::HashMap;

use re_data_store::{EntityPath, InstancePathHash};
use re_log_types::{
    component_types::{ClassId, InstanceKey, KeypointId},
    DecodedTensor, MeshId,
};
use re_renderer::{renderer::TexturedRect, Color32, OutlineMaskPreference, Size};

use crate::{
    misc::{mesh_loader::LoadedMesh, SpaceViewHighlights, TransformCache, ViewerContext},
    ui::{
        annotations::{auto_color, AnnotationMap},
        Annotations, SceneQuery,
    },
};

use super::{SpaceCamera3D, SpatialNavigationMode};

mod picking;
mod primitives;
mod scene_part;

pub use self::picking::{PickingContext, PickingHitType, PickingRayHit, PickingResult};
pub use self::primitives::SceneSpatialPrimitives;
use scene_part::ScenePart;

// ----------------------------------------------------------------------------

pub enum MeshSourceData {
    Mesh3D(re_log_types::Mesh3D),

    /// Static meshes that are embedded in the player
    ///
    /// Not used as of writing but may come back.
    #[allow(dead_code)]
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
    pub picking_instance_hash: InstancePathHash,
    // TODO(andreas): Make this Conformal3 once glow is gone?
    pub world_from_mesh: macaw::Affine3A,
    pub mesh: Arc<LoadedMesh>,
    pub outline_mask_ids: OutlineMaskPreference,
}

pub struct Image {
    /// Path to the image (note image instance ids would refer to pixels!)
    pub ent_path: EntityPath,

    pub tensor: DecodedTensor,

    /// A thing that provides additional semantic context for your dtype.
    pub annotations: Arc<Annotations>,

    /// Textured rectangle for the renderer.
    pub textured_rect: TexturedRect,
}

pub enum UiLabelTarget {
    /// Labels a given rect (in scene coordinates)
    Rect(egui::Rect),

    /// Labels a given point (in scene coordinates)
    Point2D(egui::Pos2),

    /// A point in space.
    Position3D(glam::Vec3),
}

pub struct UiLabel {
    pub text: String,
    pub color: Color32,

    /// The shape/position being labeled.
    pub target: UiLabelTarget,

    /// What is hovered if this label is hovered.
    pub labeled_instance: InstancePathHash,
}

/// Data necessary to setup the ui [`SceneSpatial`] but of no interest to `re_renderer`.
#[derive(Default)]
pub struct SceneSpatialUiData {
    pub labels: Vec<UiLabel>,

    /// Picking any any of these rects cause the referred instance to be hovered.
    /// Only use this for 2d overlays!
    pub pickable_ui_rects: Vec<(egui::Rect, InstancePathHash)>,
}

pub struct SceneSpatial {
    pub annotation_map: AnnotationMap,
    pub primitives: SceneSpatialPrimitives,
    pub ui: SceneSpatialUiData,

    /// Number of 2d primitives logged, used for heuristics.
    num_logged_2d_objects: usize,

    /// Number of 3d primitives logged, used for heuristics.
    num_logged_3d_objects: usize,

    /// All space cameras in this scene.
    /// TODO(andreas): Does this belong to [`SceneSpatialUiData`]?
    pub space_cameras: Vec<SpaceCamera3D>,
}

pub type Keypoints = HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec3>>;

impl SceneSpatial {
    pub fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        Self {
            annotation_map: Default::default(),
            primitives: SceneSpatialPrimitives::new(re_ctx),
            ui: Default::default(),
            num_logged_2d_objects: Default::default(),
            num_logged_3d_objects: Default::default(),
            space_cameras: Default::default(),
        }
    }

    /// Loads all 3D objects into the scene according to the given query.
    pub(crate) fn load(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        transforms: &TransformCache,
        highlights: &SpaceViewHighlights,
    ) {
        crate::profile_function!();

        self.annotation_map.load(ctx, query);

        let parts: Vec<&dyn ScenePart> = vec![
            &scene_part::Points3DPart { max_labels: 10 },
            // --
            &scene_part::Boxes3DPart,
            &scene_part::Lines3DPart,
            &scene_part::Arrows3DPart,
            &scene_part::MeshPart,
            &scene_part::ImagesPart,
            // --
            &scene_part::Boxes2DPart,
            // --
            // Note: Lines2DPart handles both Segments and LinesPaths since they are unified on the logging-side.
            &scene_part::Lines2DPart,
            &scene_part::Points2DPart { max_labels: 10 },
            // ---
            &scene_part::CamerasPart,
        ];

        for part in parts {
            part.load(self, ctx, query, transforms, highlights);
        }

        self.primitives.any_outlines = highlights.any_outlines();
        self.primitives.recalculate_bounding_box();
    }

    const CAMERA_COLOR: Color32 = Color32::from_rgb(150, 150, 150);

    fn load_keypoint_connections(
        &mut self,
        entity_path: &re_data_store::EntityPath,
        keypoints: Keypoints,
        annotations: &Arc<Annotations>,
    ) {
        // Generate keypoint connections if any.
        let mut line_batch = self
            .primitives
            .line_strips
            .batch("keypoint connections")
            .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

        for ((class_id, _time), keypoints_in_class) in keypoints {
            let Some(class_description) = annotations.context.class_map.get(&class_id) else {
                continue;
            };

            let color = class_description.info.color.map_or_else(
                || auto_color(class_description.info.id),
                |color| color.into(),
            );

            for (a, b) in &class_description.keypoint_connections {
                let (Some(a), Some(b)) = (keypoints_in_class.get(a), keypoints_in_class.get(b)) else {
                    re_log::warn_once!(
                        "Keypoint connection from index {:?} to {:?} could not be resolved in object {:?}",
                        a, b, entity_path
                    );
                    continue;
                };
                line_batch
                    .add_segment(*a, *b)
                    .radius(Size::AUTO)
                    .color(color)
                    // Select the entire object when clicking any of the lines.
                    .picking_instance_id(re_renderer::PickingLayerInstanceId(InstanceKey::SPLAT.0));
            }
        }
    }

    /// Heuristic whether the default way of looking at this scene should be 2d or 3d.
    pub fn preferred_navigation_mode(&self, space_info_path: &EntityPath) -> SpatialNavigationMode {
        // If there's any space cameras that are not the root, we need to go 3D, otherwise we can't display them.
        if self
            .space_cameras
            .iter()
            .any(|camera| &camera.ent_path != space_info_path)
        {
            return SpatialNavigationMode::ThreeD;
        }

        if !self.primitives.images.is_empty() {
            return SpatialNavigationMode::TwoD;
        }
        if self.num_logged_3d_objects == 0 {
            return SpatialNavigationMode::TwoD;
        }

        SpatialNavigationMode::ThreeD
    }
}
