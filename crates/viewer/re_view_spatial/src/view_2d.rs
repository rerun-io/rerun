use nohash_hasher::{IntMap, IntSet};
use re_chunk_store::MissingChunkReporter;
use re_entity_db::{EntityDb, EntityTree};
use re_log_types::EntityPath;
use re_sdk_types::blueprint::archetypes::{Background, NearClipPlane, VisualBounds2D};
use re_sdk_types::{View as _, ViewClassIdentifier};
use re_ui::{Help, UiExt as _};
use re_view::view_property_ui;
use re_viewer_context::{
    RecommendedView, ViewClass, ViewClassExt as _, ViewClassRegistryError, ViewId, ViewQuery,
    ViewSpawnHeuristics, ViewState, ViewStateExt as _, ViewSystemExecutionError, ViewerContext,
};

use crate::contexts::register_spatial_contexts;
use crate::heuristics::IndicatedVisualizableEntities;
use crate::max_image_dimension_subscriber::{ImageTypes, MaxDimensions};
use crate::shared_fallbacks;
use crate::spatial_topology::{SpatialTopology, SubSpaceConnectionFlags};
use crate::ui::SpatialViewState;
use crate::view_kind::SpatialViewKind;
use crate::visualizers::register_2d_spatial_visualizers;

#[derive(Default)]
pub struct SpatialView2D;

type ViewType = re_sdk_types::blueprint::views::Spatial2DView;

impl ViewClass for SpatialView2D {
    fn identifier() -> ViewClassIdentifier {
        ViewType::identifier()
    }

    fn display_name(&self) -> &'static str {
        "2D"
    }

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::VIEW_2D
    }

    fn help(&self, os: egui::os::OperatingSystem) -> Help {
        super::ui_2d::help(os)
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        system_registry.register_fallback_provider(Background::descriptor_kind().component, |_| {
            re_sdk_types::blueprint::components::BackgroundKind::SolidColor
        });

        fn valid_bound(rect: &egui::Rect) -> bool {
            rect.is_finite() && rect.is_positive()
        }

        system_registry.register_fallback_provider(
            VisualBounds2D::descriptor_range().component,
            |ctx| {
                let Ok(view_state) = ctx.view_state().downcast_ref::<SpatialViewState>() else {
                    return re_sdk_types::blueprint::components::VisualBounds2D::default();
                };

                // TODO(andreas): It makes sense that we query the bounding box from the view_state,
                // but the pinhole should be an ad-hoc query instead. For this we need a little bit more state information on the QueryContext.
                let default_scene_rect = view_state
                    .pinhole_at_origin
                    .as_ref()
                    .map(|pinhole| pinhole.resolution_rect())
                    .unwrap_or_else(|| {
                        // TODO(emilk): if there is a single image in this view, use that as the default bounds
                        let scene_rect_smoothed = view_state.bounding_boxes.smoothed;
                        egui::Rect::from_min_max(
                            scene_rect_smoothed.min.truncate().to_array().into(),
                            scene_rect_smoothed.max.truncate().to_array().into(),
                        )
                    });

                if valid_bound(&default_scene_rect) {
                    default_scene_rect.into()
                } else {
                    // Nothing in scene, probably.
                    re_sdk_types::blueprint::components::VisualBounds2D::default()
                }
            },
        );

        shared_fallbacks::register_fallbacks(system_registry);

        // Ensure spatial topology & max image dimension is registered.
        crate::spatial_topology::SpatialTopologyStoreSubscriber::subscription_handle();
        crate::max_image_dimension_subscriber::MaxImageDimensionsStoreSubscriber::subscription_handle();

        register_spatial_contexts(system_registry)?;
        register_2d_spatial_visualizers(system_registry)?;

        Ok(())
    }

    fn new_state(&self) -> Box<dyn ViewState> {
        Box::<SpatialViewState>::default()
    }

    fn preferred_tile_aspect_ratio(&self, state: &dyn ViewState) -> Option<f32> {
        state.downcast_ref::<SpatialViewState>().ok().map(|state| {
            let (width, height) = state.visual_bounds_2d.map_or_else(
                || {
                    let bbox = &state.bounding_boxes.smoothed;
                    (
                        (bbox.max.x - bbox.min.x).abs(),
                        (bbox.max.y - bbox.min.y).abs(),
                    )
                },
                |bounds| {
                    (
                        bounds.x_range.abs_len() as f32,
                        bounds.y_range.abs_len() as f32,
                    )
                },
            );

            width / height
        })
    }

    fn supports_visible_time_range(&self) -> bool {
        true
    }

    fn layout_priority(&self) -> re_viewer_context::ViewClassLayoutPriority {
        re_viewer_context::ViewClassLayoutPriority::High
    }

    fn recommended_origin_for_entities(
        &self,
        entities: &IntSet<EntityPath>,
        entity_db: &EntityDb,
    ) -> Option<EntityPath> {
        let common_ancestor = EntityPath::common_ancestor_of(entities.iter());

        // For a 2D view, the origin of the subspace defined by the common ancestor is always
        // better.
        SpatialTopology::access(entity_db.store_id(), |topo| {
            topo.subspace_for_entity(&common_ancestor).origin.clone()
        })
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
        include_entity: &dyn Fn(&EntityPath) -> bool,
    ) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();

        let IndicatedVisualizableEntities {
            indicated_entities,
            excluded_entities,
        } = IndicatedVisualizableEntities::new(
            ctx,
            Self::identifier(),
            SpatialViewKind::TwoD,
            include_entity,
            |_| {},
        );

        let image_dimensions =
            crate::max_image_dimension_subscriber::MaxImageDimensionsStoreSubscriber::access(
                ctx.store_id(),
                |image_dimensions| image_dimensions.clone(),
            )
            .unwrap_or_default();

        // Spawn a view at each subspace that has any potential 2D content.
        // Note that visualizability filtering is all about being in the right subspace,
        // so we don't need to call the visualizers' filter functions here.
        SpatialTopology::access(ctx.store_id(), |topo| {
            ViewSpawnHeuristics::new(topo.iter_subspaces().flat_map(|subspace| {
                if !subspace.supports_2d_content()
                    || subspace.entities.is_empty()
                    || indicated_entities.is_disjoint(&subspace.entities)
                {
                    return Vec::new();
                }

                // Collect just the 2D-relevant entities in this subspace
                let relevant_entities: IntSet<EntityPath> = subspace
                    .entities
                    .iter()
                    .filter(|e| indicated_entities.contains(e))
                    .cloned()
                    .collect();

                // For explicit 2D spaces with a pinhole at the origin, otherwise start at the common ancestor.
                // This generally avoids the `/` root entity unless it's required as a common ancestor.
                let recommended_root = if subspace
                    .connection_to_parent
                    .contains(SubSpaceConnectionFlags::Pinhole)
                {
                    subspace.origin.clone()
                } else {
                    EntityPath::common_ancestor_of(relevant_entities.iter())
                };

                let mut recommended_views = Vec::<RecommendedView>::new();

                recommended_views_with_image_splits(
                    ctx,
                    &image_dimensions,
                    &recommended_root,
                    &relevant_entities,
                    &mut recommended_views,
                );

                if recommended_views.is_empty() {
                    // There were apparently no images, so just create a single space-view at the common root:
                    recommended_views.push(RecommendedView::new_subtree(recommended_root));
                }

                // Since we don't track the transform frames created by explicit
                // coordinate frames, we can't make assumptions about the tree if
                // there are any explicit coordinate frames.
                if !topo.has_explicit_coordinate_frame() {
                    for recommended_view in &mut recommended_views {
                        recommended_view.exclude_entities(&excluded_entities);
                    }
                }

                recommended_views
            }))
        })
        .unwrap_or_else(ViewSpawnHeuristics::empty)
    }

    fn selection_ui(
        &self,
        ctx: &re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<SpatialViewState>()?;
        // TODO(andreas): list_item'ify the rest
        ui.selection_grid("spatial_settings_ui").show(ui, |ui| {
            state.bounding_box_ui(ui, SpatialViewKind::TwoD);
        });

        re_ui::list_item::list_item_scope(ui, "spatial_view2d_selection_ui", |ui| {
            let view_ctx = self.view_context(ctx, view_id, state, space_origin);
            view_property_ui::<VisualBounds2D>(&view_ctx, ui);
            view_property_ui::<NearClipPlane>(&view_ctx, ui);
            view_property_ui::<Background>(&view_ctx, ui);
        });

        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        missing_chunk_reporter: &MissingChunkReporter,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let state = state.downcast_mut::<SpatialViewState>()?;
        state.update_frame_statistics(ui, &system_output, SpatialViewKind::TwoD);

        self.view_2d(ctx, missing_chunk_reporter, ui, state, query, system_output)
    }
}

#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
struct NonNestedImageCounts {
    color: usize,
    depth: usize,
    segmentation: usize,
}

impl NonNestedImageCounts {
    fn total(&self) -> usize {
        let Self {
            color,
            depth,
            segmentation,
        } = self;
        color + depth + segmentation
    }

    fn has_any_images(&self) -> bool {
        self.total() > 0
    }

    fn increment_count(&mut self, dims: &MaxDimensions) {
        self.color += dims.image_types.contains(ImageTypes::IMAGE) as usize
            + dims.image_types.contains(ImageTypes::ENCODED_IMAGE) as usize
            + dims.image_types.contains(ImageTypes::VIDEO_ASSET) as usize
            + dims.image_types.contains(ImageTypes::VIDEO_STREAM) as usize;
        self.depth += dims.image_types.contains(ImageTypes::DEPTH_IMAGE) as usize
            + dims.image_types.contains(ImageTypes::ENCODED_DEPTH_IMAGE) as usize;
        self.segmentation += dims.image_types.contains(ImageTypes::SEGMENTATION_IMAGE) as usize;
    }
}

// Find the shared image dimensions of every image-entity that is not
// not nested under another image.
fn has_single_shared_image_dimension(
    image_dimensions: &IntMap<EntityPath, MaxDimensions>,
    subtree: &EntityTree,
    non_nested_image_counts: &mut NonNestedImageCounts,
) -> bool {
    let mut image_dimension = None;

    let mut pending_subtrees = vec![subtree];

    while let Some(subtree) = pending_subtrees.pop() {
        if let Some(dimensions) = image_dimensions.get(&subtree.path) {
            let new_dimension = [dimensions.height, dimensions.width];
            if let Some(existing_dimension) = image_dimension {
                if existing_dimension != new_dimension {
                    return false;
                }
            } else {
                image_dimension = Some(new_dimension);
            }

            non_nested_image_counts.increment_count(dimensions);
        }

        // We always need to recurse to check if child entities have images of different size
        pending_subtrees.extend(subtree.children.values());
    }

    image_dimension.is_some()
}

fn recommended_views_with_image_splits(
    ctx: &ViewerContext<'_>,
    image_dimensions: &IntMap<EntityPath, MaxDimensions>,
    recommended_origin: &EntityPath,
    visualizable_entities: &IntSet<EntityPath>,
    recommended: &mut Vec<RecommendedView>,
) {
    re_tracing::profile_function!();

    let tree = ctx.recording().tree();

    let Some(subtree) = tree.subtree(recommended_origin) else {
        re_log::debug_warn_once!("Ancestor of entity not found in entity tree.");
        return;
    };

    let mut image_counts = NonNestedImageCounts::default();

    // Note that since this only finds entities with image dimensions, it naturally filters for `visualizable_entities`.
    let all_have_same_size =
        has_single_shared_image_dimension(image_dimensions, subtree, &mut image_counts);

    if !image_counts.has_any_images() {
        // This utility is all about finding views with *image* splits.
        // If there's no images in this subtree, we're done.
        return;
    }

    // Should we create an all-inclusive (subtree) view at this path?
    // We usually want to be as inclusive as possible (i.e. put as much as possible in the same view)
    // as long as the image contents (recursively) are _compatible_.
    // Compatible images are images that can be overlaied on top of each other productively, e.g.:
    // * Stack a depth image on top of an RGB image
    // * Stack multiple segmentation images on top of an RGB and/or depth image
    //
    // NOTE: we allow stacking multiple segmentation images, since that can be quite useful sometimes.
    // We also allow stacking ONE depth image on ONE color image, but never multiple of either.
    //
    // Of course the images must be of the same size, or stacking does not make sense.
    //
    // Note that non-image entities (e.g. bounding rectangles) may be present,
    // but they are always assumed to be compatible with any image.

    let could_have_subtree_view =
        all_have_same_size && image_counts.color <= 1 && image_counts.depth <= 1;

    if could_have_subtree_view && visualizable_entities.contains(recommended_origin) {
        // The entity is itself visualizable, so it makes sense to have it as an origin.
        recommended.push(RecommendedView::new_subtree(recommended_origin.clone()));
        return; // We now include everything below this subtree, so we can stop here.
    }

    // We may still want to create a subtree view here, but only if it is not _too_ general.
    // For instance, if the only data is at `/a/b/c` we want to create
    // a view with the `origin: /a/b/c`, NOT `origin: /a/**`.
    // So we recurse on the children and see if it would result in a single view, or multiple:

    let num_recommended_before = recommended.len();

    if visualizable_entities.contains(recommended_origin) {
        // If the root also had a visualizable entity, give it its own space.
        // TODO(jleibs): Maybe merge this entity into each child
        recommended.push(RecommendedView::new_single_entity(
            recommended_origin.clone(),
        ));
    }

    // Recurse into the children:
    for child in subtree.children.values() {
        recommended_views_with_image_splits(
            ctx,
            image_dimensions,
            &child.path,
            visualizable_entities,
            recommended,
        );
    }

    let num_children_added = recommended.len() - num_recommended_before;

    if could_have_subtree_view {
        if num_children_added <= 1 {
            // A recursive view would have been too general - keep the child!
            // That is: better to recommend `/recommended_origin/only_child/**` over
            // `/recommended_origin/**`
        } else {
            // Better to only add /recommended_origin/** than recommended_origin/a/**, recommended_origin/b/**, etc
            recommended.truncate(num_recommended_before);
            recommended.push(RecommendedView::new_subtree(recommended_origin.clone()));
        }
    }
}
