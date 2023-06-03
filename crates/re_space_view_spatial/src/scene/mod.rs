mod contexts;
mod parts;
mod picking;
mod primitives;

pub use contexts::{SpatialSceneContext, TransformContext, UnreachableTransform};
pub use parts::{SpatialScenePartCollection, SpatialScenePartData};
pub use picking::{PickingContext, PickingHitType, PickingRayHit, PickingResult};
pub use primitives::SceneSpatialPrimitives;

use ahash::HashMap;

use re_components::{ClassId, DecodedTensor, DrawOrder, InstanceKey, KeypointId};
use re_data_store::{EntityPath, InstancePathHash};
use re_log_types::EntityPathHash;
use re_renderer::{renderer::TexturedRect, Color32, Size};
use re_viewer_context::{
    auto_color, AnnotationMap, Scene, ScenePartCollection, SceneQuery, SpaceViewHighlights,
    TypedScene, ViewerContext,
};

use crate::{space_camera_3d::SpaceCamera3D, SpatialSpaceViewClass};

use super::SpatialNavigationMode;

use self::contexts::SpatialSceneEntityContext;

use contexts::EntityDepthOffsets;

const SIZE_BOOST_IN_POINTS_FOR_LINE_OUTLINES: f32 = 1.5;
const SIZE_BOOST_IN_POINTS_FOR_POINT_OUTLINES: f32 = 2.5;

pub struct Image {
    /// Path to the image (note image instance ids would refer to pixels!)
    pub ent_path: EntityPath,

    pub tensor: DecodedTensor,

    /// Textured rectangle for the renderer.
    pub textured_rect: TexturedRect,

    /// Pinhole camera this image is under.
    pub parent_pinhole: Option<EntityPathHash>,

    /// Draw order value used.
    pub draw_order: DrawOrder,
}

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

    // TODO(andreas): Temporary field. The hosting struct will be removed once SpatialScene is fully ported to the SpaceViewClass framework.
    pub scene: TypedScene<SpatialSpaceViewClass>,
    pub draw_data: Vec<re_renderer::QueueableDrawData>,
}

pub type Keypoints = HashMap<(ClassId, i64), HashMap<KeypointId, glam::Vec3>>;

impl EntityDepthOffsets {
    pub fn get(&self, ent_path: &EntityPath) -> Option<re_renderer::DepthOffset> {
        self.per_entity.get(&ent_path.hash()).cloned()
    }
}

impl SceneSpatial {
    pub fn new(re_ctx: &mut re_renderer::RenderContext) -> Self {
        Self {
            annotation_map: Default::default(),
            primitives: SceneSpatialPrimitives::new(re_ctx),
            ui: Default::default(),
            // TODO(andreas): Workaround for not having default on `Scene`. Soon not needed anyways
            scene: Default::default(),
            draw_data: Default::default(),
        }
    }

    /// Loads all 3D objects into the scene according to the given query.
    pub fn load(
        &mut self,
        ctx: &mut ViewerContext<'_>,
        query: &SceneQuery<'_>,
        highlights: SpaceViewHighlights,
    ) {
        re_tracing::profile_function!();

        self.annotation_map.load(ctx, query);

        let parts: Vec<&dyn parts::ScenePart> = vec![&parts::ImagesPart];

        // TODO(wumpf): Temporary build up of scene. This will be handled by the SpaceViewClass framework later.
        let mut scene = TypedScene::<SpatialSpaceViewClass> {
            context: SpatialSceneContext::default(),
            parts: SpatialScenePartCollection::default(),
            highlights: Default::default(),
        };
        self.draw_data =
            scene.populate(ctx, query, &re_space_view::EmptySpaceViewState, highlights);

        for part in parts {
            part.load(
                self,
                ctx,
                query,
                &scene.context.transforms,
                &scene.highlights,
                &scene.context.depth_offsets,
            );
        }

        self.primitives.any_outlines = scene.highlights.any_outlines();
        self.primitives.recalculate_bounding_box();

        for scene_part in scene.parts.vec_mut() {
            if let Some(data) = scene_part.data() {
                self.ui.labels.extend(data.ui_labels.iter().cloned());
                self.primitives.bounding_box =
                    self.primitives.bounding_box.union(data.bounding_box);
            }
        }

        self.draw_data.extend(
            scene
                .context
                .shared_render_builders
                .lines
                .take()
                .and_then(|l| match l.into_inner().to_draw_data(ctx.render_ctx) {
                    Ok(d) => Some(d.into()),
                    Err(err) => {
                        re_log::error_once!("Failed to build line strip draw data: {err}");
                        None
                    }
                }),
        );
        self.draw_data.extend(
            scene
                .context
                .shared_render_builders
                .points
                .take()
                .and_then(|l| match l.into_inner().to_draw_data(ctx.render_ctx) {
                    Ok(d) => Some(d.into()),
                    Err(err) => {
                        re_log::error_once!("Failed to build point draw data: {err}");
                        None
                    }
                }),
        );

        self.scene = scene;
    }

    const CAMERA_COLOR: Color32 = Color32::from_rgb(150, 150, 150);

    pub fn space_cameras(&self) -> &[SpaceCamera3D] {
        &self.scene.parts.cameras.space_cameras
    }

    /// Heuristic whether the default way of looking at this scene should be 2d or 3d.
    pub fn preferred_navigation_mode(&self, space_info_path: &EntityPath) -> SpatialNavigationMode {
        // If there's any space cameras that are not the root, we need to go 3D, otherwise we can't display them.
        if self
            .space_cameras()
            .iter()
            .any(|camera| &camera.ent_path != space_info_path)
        {
            return SpatialNavigationMode::ThreeD;
        }

        if !self.primitives.images.is_empty() {
            return SpatialNavigationMode::TwoD;
        }

        if self
            .scene
            .context
            .num_3d_primitives
            .load(std::sync::atomic::Ordering::Relaxed)
            == 0
        {
            return SpatialNavigationMode::TwoD;
        }

        SpatialNavigationMode::ThreeD
    }
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
