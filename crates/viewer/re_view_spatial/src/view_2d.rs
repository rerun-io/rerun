use ahash::HashSet;
use nohash_hasher::{IntMap, IntSet};

use re_entity_db::{EntityDb, EntityTree};
use re_log_types::EntityPath;
use re_types::View;
use re_types::{
    archetypes::{DepthImage, Image},
    blueprint::archetypes::{Background, NearClipPlane, VisualBounds2D},
    Archetype, ComponentName, ViewClassIdentifier,
};
use re_ui::UiExt as _;
use re_view::view_property_ui;
use re_viewer_context::{
    RecommendedView, ViewClass, ViewClassRegistryError, ViewId, ViewQuery, ViewSpawnHeuristics,
    ViewState, ViewStateExt as _, ViewSystemExecutionError, ViewerContext,
    VisualizableFilterContext,
};

use crate::{
    contexts::register_spatial_contexts,
    heuristics::default_visualized_entities_for_visualizer_kind,
    max_image_dimension_subscriber::MaxDimensions,
    spatial_topology::{SpatialTopology, SubSpaceConnectionFlags},
    ui::SpatialViewState,
    view_kind::SpatialViewKind,
    visualizers::register_2d_spatial_visualizers,
};

#[derive(Default)]
pub struct VisualizableFilterContext2D {
    // TODO(andreas): Would be nice to use `EntityPathHash` in order to avoid bumping reference counters.
    pub entities_in_main_2d_space: IntSet<EntityPath>,
    pub reprojectable_3d_entities: IntSet<EntityPath>,
}

impl VisualizableFilterContext for VisualizableFilterContext2D {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Default)]
pub struct SpatialView2D;

type ViewType = re_types::blueprint::views::Spatial2DView;

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

    fn help_markdown(&self, egui_ctx: &egui::Context) -> String {
        super::ui_2d::help_markdown(egui_ctx)
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::ViewSystemRegistrator<'_>,
    ) -> Result<(), ViewClassRegistryError> {
        // Ensure spatial topology & max image dimension is registered.
        crate::spatial_topology::SpatialTopologyStoreSubscriber::subscription_handle();
        crate::transform_component_tracker::TransformComponentTrackerStoreSubscriber::subscription_handle();
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

    fn recommended_root_for_entities(
        &self,
        entities: &IntSet<EntityPath>,
        entity_db: &EntityDb,
    ) -> Option<EntityPath> {
        let common_ancestor = EntityPath::common_ancestor_of(entities.iter());

        // For a 2D view, the origin of the subspace defined by the common ancestor is always
        // better.
        SpatialTopology::access(&entity_db.store_id(), |topo| {
            topo.subspace_for_entity(&common_ancestor).origin.clone()
        })
    }

    fn visualizable_filter_context(
        &self,
        space_origin: &EntityPath,
        entity_db: &re_entity_db::EntityDb,
    ) -> Box<dyn VisualizableFilterContext> {
        re_tracing::profile_function!();

        // TODO(andreas): The `VisualizableFilterContext` depends entirely on the spatial topology.
        // If the topology hasn't changed, we don't need to recompute any of this.
        // Also, we arrive at the same `VisualizableFilterContext` for lots of different origins!

        let context = SpatialTopology::access(&entity_db.store_id(), |topo| {
            let primary_space = topo.subspace_for_entity(space_origin);
            if !primary_space.supports_2d_content() {
                // If this is strict 3D space, only display the origin entity itself.
                // Everything else we have to assume requires some form of transformation.
                return VisualizableFilterContext2D {
                    entities_in_main_2d_space: std::iter::once(space_origin.clone()).collect(),
                    reprojectable_3d_entities: Default::default(),
                };
            }

            // All space are visualizable + the parent space if it is connected via a pinhole.
            // For the moment we don't allow going down pinholes again.
            let reprojectable_3d_entities = if primary_space
                .connection_to_parent
                .contains(SubSpaceConnectionFlags::Pinhole)
            {
                topo.subspace_for_subspace_origin(primary_space.parent_space)
                    .map(|parent_space| parent_space.entities.clone())
                    .unwrap_or_default()
            } else {
                Default::default()
            };

            VisualizableFilterContext2D {
                entities_in_main_2d_space: primary_space.entities.clone(),
                reprojectable_3d_entities,
            }
        });

        Box::new(context.unwrap_or_default())
    }

    fn spawn_heuristics(&self, ctx: &ViewerContext<'_>) -> re_viewer_context::ViewSpawnHeuristics {
        re_tracing::profile_function!();

        let indicated_entities = default_visualized_entities_for_visualizer_kind(
            ctx,
            Self::identifier(),
            SpatialViewKind::TwoD,
        );

        let image_dimensions =
            crate::max_image_dimension_subscriber::MaxImageDimensionsStoreSubscriber::access(
                &ctx.recording_id(),
                |image_dimensions| image_dimensions.clone(),
            )
            .unwrap_or_default();

        // Spawn a view at each subspace that has any potential 2D content.
        // Note that visualizability filtering is all about being in the right subspace,
        // so we don't need to call the visualizers' filter functions here.
        SpatialTopology::access(&ctx.recording_id(), |topo| {
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

                recommended_views
            }))
        })
        .unwrap_or_default()
    }

    fn selection_ui(
        &self,
        ctx: &re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,
        _space_origin: &EntityPath,
        view_id: ViewId,
    ) -> Result<(), ViewSystemExecutionError> {
        let state = state.downcast_mut::<SpatialViewState>()?;
        // TODO(andreas): list_item'ify the rest
        ui.selection_grid("spatial_settings_ui").show(ui, |ui| {
            state.bounding_box_ui(ui, SpatialViewKind::TwoD);
        });

        re_ui::list_item::list_item_scope(ui, "spatial_view2d_selection_ui", |ui| {
            view_property_ui::<VisualBounds2D>(ctx, ui, view_id, self, state);
            view_property_ui::<NearClipPlane>(ctx, ui, view_id, self, state);
            view_property_ui::<Background>(ctx, ui, view_id, self, state);
        });

        Ok(())
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut dyn ViewState,

        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), ViewSystemExecutionError> {
        re_tracing::profile_function!();

        let state = state.downcast_mut::<SpatialViewState>()?;
        state.update_frame_statistics(ui, &system_output, SpatialViewKind::TwoD);

        self.view_2d(ctx, ui, state, query, system_output)
    }
}

// Count the number of image entities with the given component exist that aren't
// children of other entities in the bucket.
fn count_non_nested_images_with_component(
    image_dimensions: &IntMap<EntityPath, MaxDimensions>,
    entity_bucket: &IntSet<EntityPath>,
    entity_db: &re_entity_db::EntityDb,
    subtree: &EntityTree,
    component_name: &ComponentName,
) -> usize {
    if image_dimensions.contains_key(&subtree.path) {
        // bool true -> 1
        entity_db
            .storage_engine()
            .store()
            .entity_has_component(&subtree.path, component_name) as usize
    } else if !entity_bucket
        .iter()
        .any(|e| e.is_descendant_of(&subtree.path))
    {
        0 // early-out optimization
    } else {
        subtree
            .children
            .values()
            .map(|child| {
                count_non_nested_images_with_component(
                    image_dimensions,
                    entity_bucket,
                    entity_db,
                    child,
                    component_name,
                )
            })
            .sum()
    }
}

// Find the image dimensions of every image-entity in the bucket that is not
// not nested under another image.
//
// We track a set of just height/width as different channels could be allowed to
// stack.
fn find_non_nested_image_dimensions(
    image_dimensions: &IntMap<EntityPath, MaxDimensions>,
    entity_bucket: &IntSet<EntityPath>,
    subtree: &EntityTree,
    found_image_dimensions: &mut HashSet<[u64; 2]>,
) {
    if let Some(dimensions) = image_dimensions.get(&subtree.path) {
        // If we found an image entity, add its dimensions to the set.
        found_image_dimensions.insert([dimensions.height as _, dimensions.width as _]);
    } else if entity_bucket
        .iter()
        .any(|e| e.is_descendant_of(&subtree.path))
    {
        // Otherwise recurse
        for child in subtree.children.values() {
            find_non_nested_image_dimensions(
                image_dimensions,
                entity_bucket,
                child,
                found_image_dimensions,
            );
        }
    }
}

fn recommended_views_with_image_splits(
    ctx: &ViewerContext<'_>,
    image_dimensions: &IntMap<EntityPath, MaxDimensions>,
    recommended_root: &EntityPath,
    entities: &IntSet<EntityPath>,
    recommended: &mut Vec<RecommendedView>,
) {
    re_tracing::profile_function!();

    let tree = ctx.recording().tree();

    let Some(subtree) = tree.subtree(recommended_root) else {
        if cfg!(debug_assertions) {
            re_log::warn_once!("Ancestor of entity not found in entity tree.");
        }
        return;
    };

    let mut found_image_dimensions = Default::default();

    find_non_nested_image_dimensions(
        image_dimensions,
        entities,
        subtree,
        &mut found_image_dimensions,
    );

    use re_types::ComponentBatch as _;
    let image_count = count_non_nested_images_with_component(
        image_dimensions,
        entities,
        ctx.recording(),
        subtree,
        &Image::indicator().name(),
    );

    let depth_count = count_non_nested_images_with_component(
        image_dimensions,
        entities,
        ctx.recording(),
        subtree,
        &DepthImage::indicator().name(),
    );

    let video_count = count_non_nested_images_with_component(
        image_dimensions,
        entities,
        ctx.recording(),
        subtree,
        &re_types::archetypes::AssetVideo::indicator().name(),
    );

    let all_have_same_size = found_image_dimensions.len() <= 1;

    // NOTE: we allow stacking segmentation images, since that can be quite useful sometimes.
    let overlap = all_have_same_size && image_count + video_count <= 1 && depth_count <= 1;

    if overlap {
        // If there are multiple images of the same size but of different types, then we can overlap them on top of each other.
        // This can be useful for comparing a segmentation image on top of an RGB image, for instance.
        recommended.push(RecommendedView::new_subtree(recommended_root.clone()));
    } else {
        // Split the space and recurse

        // If the root also had a visualizable entity, give it its own space.
        // TODO(jleibs): Maybe merge this entity into each child
        if entities.contains(recommended_root) {
            recommended.push(RecommendedView::new_single_entity(recommended_root.clone()));
        }

        // And then recurse into the children
        for child in subtree.children.values() {
            let sub_bucket: IntSet<_> = entities
                .iter()
                .filter(|e| e.starts_with(&child.path))
                .cloned()
                .collect();

            if !sub_bucket.is_empty() {
                recommended_views_with_image_splits(
                    ctx,
                    image_dimensions,
                    &child.path,
                    &sub_bucket,
                    recommended,
                );
            }
        }
    }
}
