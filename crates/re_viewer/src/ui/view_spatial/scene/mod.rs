use std::sync::Arc;

use ahash::HashMap;
use re_data_store::{EntityPath, InstancePathHash};
use re_log_types::{
    component_types::{ClassId, KeypointId, Tensor},
    MeshId,
};
use re_renderer::{Color32, Size};

use super::{eye::Eye, SpaceCamera3D, SpatialNavigationMode};
use crate::{
    misc::{
        mesh_loader::LoadedMesh, HoverHighlight, InteractionHighlight, SelectionHighlight,
        SpaceViewHighlights, TransformCache, ViewerContext,
    },
    ui::{
        annotations::{auto_color, AnnotationMap},
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
    pub instance_path_hash: InstancePathHash,
    // TODO(andreas): Make this Conformal3 once glow is gone?
    pub world_from_mesh: macaw::Affine3A,
    pub mesh: Arc<LoadedMesh>,
    pub additive_tint: Color32,
}

pub struct Image {
    pub instance_path_hash: InstancePathHash,

    pub tensor: Tensor,

    /// If this is a depth map, how long is a meter?
    ///
    /// For example, with a `u16` dtype one might have
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
    pub labled_instance: InstancePathHash,
}

pub struct Label3D {
    pub(crate) text: String,

    /// Origin of the label
    pub(crate) origin: glam::Vec3,
}

/// Data necessary to setup the ui [`SceneSpatial`] but of no interest to `re_renderer`.
#[derive(Default)]
pub struct SceneSpatialUiData {
    pub labels_3d: Vec<Label3D>,
    pub labels_2d: Vec<Label2D>,

    /// Picking any any of these rects cause the referred instance to be hovered.
    /// Only use this for 2d overlays!
    pub pickable_ui_rects: Vec<(egui::Rect, InstancePathHash)>,

    /// Images are a special case of rects where we're storing some extra information to allow miniature previews etc.
    pub images: Vec<Image>,
}

#[derive(Default)]
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

fn instance_path_hash_if_interactive(
    entity_path: &EntityPath,
    interactive: bool,
) -> InstancePathHash {
    if interactive {
        InstancePathHash::entity_splat(entity_path)
    } else {
        InstancePathHash::NONE
    }
}

pub type Keypoints = HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec3>>;

impl SceneSpatial {
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
            &scene_part::Points2DPart,
            // ---
            &scene_part::CamerasPart,
        ];

        for part in parts {
            part.load(self, ctx, query, transforms, highlights);
        }

        self.primitives.recalculate_bounding_boxes();
    }

    // TODO(andreas): Better ways to determine these?
    const HOVER_COLOR: Color32 = Color32::from_rgb(255, 200, 200);
    const SELECTION_COLOR: Color32 = Color32::from_rgb(255, 170, 170);
    const SIBLING_SELECTION_COLOR: Color32 = Color32::from_rgb(255, 140, 140);
    const CAMERA_COLOR: Color32 = Color32::from_rgb(150, 150, 150);

    fn size_boost(size: Size) -> Size {
        if size.is_auto() {
            Size::AUTO_LARGE
        } else {
            size * 1.33
        }
    }

    fn apply_hover_and_selection_effect(
        size: &mut Size,
        color: &mut Color32,
        highlight: InteractionHighlight,
    ) {
        // TODO(#889):
        // We want to use outlines instead of color highlighting, but this is a bigger endeavour, so for now:

        let mut highlight_color = *color;
        if highlight.selection.is_some() {
            *size = Self::size_boost(*size);
            highlight_color = match highlight.selection {
                SelectionHighlight::None => unreachable!(),
                SelectionHighlight::SiblingSelection => Self::SIBLING_SELECTION_COLOR,
                SelectionHighlight::Selection => Self::SELECTION_COLOR,
            };
        }
        match highlight.hover {
            HoverHighlight::None => {}
            HoverHighlight::Hovered => {
                highlight_color = Self::HOVER_COLOR;
            }
        }

        if highlight.is_some() {
            // Interpolate with factor 2/3 towards the highlight color (in gamma space for speed)
            *color = Color32::from_rgba_premultiplied(
                ((color.r() as u32 + highlight_color.r() as u32 * 2) / 3) as u8,
                ((color.g() as u32 + highlight_color.g() as u32 * 2) / 3) as u8,
                ((color.b() as u32 + highlight_color.b() as u32 * 2) / 3) as u8,
                color.a(),
            );
        }
    }

    fn apply_hover_and_selection_effect_color(
        color: Color32,
        highlight: InteractionHighlight,
    ) -> Color32 {
        let mut color = color;
        // (counting on inlining to remove unused fields!)
        Self::apply_hover_and_selection_effect(&mut Size::AUTO.clone(), &mut color, highlight);
        color
    }

    fn apply_hover_and_selection_effect_size(size: Size, highlight: InteractionHighlight) -> Size {
        let mut size = size;
        // (counting on inlining to remove unused fields!)
        Self::apply_hover_and_selection_effect(&mut size, &mut Color32::WHITE.clone(), highlight);
        size
    }

    fn load_keypoint_connections(
        &mut self,
        entity_path: &re_data_store::EntityPath,
        keypoints: Keypoints,
        annotations: &Arc<Annotations>,
        interactive: bool,
    ) {
        // Generate keypoint connections if any.
        let instance_path_hash = instance_path_hash_if_interactive(entity_path, interactive);

        let mut line_batch = self.primitives.line_strips.batch("keypoint connections");

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
                    .user_data(instance_path_hash);
            }
        }
    }

    /// Heuristic whether the default way of looking at this scene should be 2d or 3d.
    pub fn preferred_navigation_mode(&self, space_info_path: &EntityPath) -> SpatialNavigationMode {
        // If there's any space cameras that are not the root, we need to go 3D, otherwise we can't display them.
        if self
            .space_cameras
            .iter()
            .any(|camera| &camera.entity_path != space_info_path)
        {
            return SpatialNavigationMode::ThreeD;
        }

        if !self.ui.images.is_empty() {
            return SpatialNavigationMode::TwoD;
        }
        if self.num_logged_3d_objects == 0 {
            return SpatialNavigationMode::TwoD;
        }

        SpatialNavigationMode::ThreeD
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
