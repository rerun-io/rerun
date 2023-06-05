mod contexts;
mod parts;
mod picking;

pub use contexts::{SpatialSceneContext, TransformContext};
pub use parts::{SpatialScenePartCollection, SpatialScenePartData};
pub use picking::{PickableUiRect, PickingContext, PickingHitType, PickingRayHit, PickingResult};

use ahash::HashMap;

use re_components::{ClassId, InstanceKey, KeypointId};
use re_data_store::{EntityPath, InstancePathHash};
use re_renderer::{Color32, Size};
use re_viewer_context::{auto_color, TypedScene};

use crate::{ui::SpatialNavigationMode, SpatialSpaceView};

use self::contexts::SpatialSceneEntityContext;

pub const SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES: f32 = 1.5;
const SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES: f32 = 2.5;

#[derive(Clone)]
pub enum UiLabelTarget {
    /// Labels a given rect (in scene coordinates)
    Rect(egui::Rect),

    /// Labels a given point (in scene coordinates)
    Point2D(egui::Pos2),

    /// A point in space.
    Position3D(glam::Vec3),
}

#[derive(Clone)]
pub struct UiLabel {
    pub text: String,
    pub color: Color32,

    /// The shape/position being labeled.
    pub target: UiLabelTarget,

    /// What is hovered if this label is hovered.
    pub labeled_instance: InstancePathHash,
}

pub type SceneSpatial = TypedScene<SpatialSpaceView>;
pub type Keypoints = HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec3>>;

/// Heuristic whether the default way of looking at this scene should be 2d or 3d.
pub fn preferred_navigation_mode(
    scene: &SceneSpatial,
    space_info_path: &EntityPath,
) -> SpatialNavigationMode {
    // If there's any space cameras that are not the root, we need to go 3D, otherwise we can't display them.
    if scene
        .parts
        .cameras
        .space_cameras
        .iter()
        .any(|camera| &camera.ent_path != space_info_path)
    {
        return SpatialNavigationMode::ThreeD;
    }

    if !scene.parts.images.images.is_empty() {
        return SpatialNavigationMode::TwoD;
    }

    if scene
        .context
        .num_3d_primitives
        .load(std::sync::atomic::Ordering::Relaxed)
        == 0
    {
        return SpatialNavigationMode::TwoD;
    }

    SpatialNavigationMode::ThreeD
}

pub fn load_keypoint_connections(
    ent_context: &SpatialSceneEntityContext<'_>,
    entity_path: &re_data_store::EntityPath,
    keypoints: Keypoints,
) {
    if keypoints.is_empty() {
        return;
    }

    // Generate keypoint connections if any.
    let mut line_builder = ent_context.shared_render_builders.lines();
    let mut line_batch = line_builder
        .batch("keypoint connections")
        .picking_object_id(re_renderer::PickingLayerObjectId(entity_path.hash64()));

    for ((class_id, _time), keypoints_in_class) in keypoints {
        let Some(class_description) = ent_context.annotations.context.class_map.get(&class_id) else {
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
                .flags(re_renderer::renderer::LineStripFlags::FLAG_COLOR_GRADIENT)
                // Select the entire object when clicking any of the lines.
                .picking_instance_id(re_renderer::PickingLayerInstanceId(InstanceKey::SPLAT.0));
        }
    }
}
