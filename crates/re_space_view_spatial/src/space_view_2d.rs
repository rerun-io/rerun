use ahash::HashSet;
use nohash_hasher::{IntMap, IntSet};

use re_entity_db::{EntityProperties, EntityTree};
use re_log_types::{EntityPath, EntityPathFilter};
use re_types::{
    archetypes::{DepthImage, Image},
    Archetype, ComponentName,
};
use re_viewer_context::{
    PerSystemEntities, RecommendedSpaceView, SpaceViewClass, SpaceViewClassRegistryError,
    SpaceViewId, SpaceViewSpawnHeuristics, SpaceViewSystemExecutionError, ViewQuery, ViewerContext,
    VisualizableFilterContext,
};

use crate::{
    contexts::{register_spatial_contexts, PrimitiveCounter},
    heuristics::{
        default_visualized_entities_for_visualizer_kind, update_object_property_heuristics,
    },
    max_image_dimension_subscriber::{ImageDimensions, MaxImageDimensions},
    spatial_topology::{SpatialTopology, SubSpaceConnectionFlags},
    ui::SpatialSpaceViewState,
    view_kind::SpatialSpaceViewKind,
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
pub struct SpatialSpaceView2D;

impl SpaceViewClass for SpatialSpaceView2D {
    type State = SpatialSpaceViewState;

    const IDENTIFIER: &'static str = "2D";
    const DISPLAY_NAME: &'static str = "2D";

    fn icon(&self) -> &'static re_ui::Icon {
        &re_ui::icons::SPACE_VIEW_2D
    }

    fn help_text(&self, re_ui: &re_ui::ReUi) -> egui::WidgetText {
        super::ui_2d::help_text(re_ui)
    }

    fn on_register(
        &self,
        system_registry: &mut re_viewer_context::SpaceViewSystemRegistrator<'_>,
    ) -> Result<(), SpaceViewClassRegistryError> {
        // Ensure spatial topology is registered.
        crate::spatial_topology::SpatialTopologyStoreSubscriber::subscription_handle();

        register_spatial_contexts(system_registry)?;
        register_2d_spatial_visualizers(system_registry)?;

        Ok(())
    }

    fn preferred_tile_aspect_ratio(&self, state: &Self::State) -> Option<f32> {
        let size = state.bounding_boxes.accumulated.size();
        Some(size.x / size.y)
    }

    fn layout_priority(&self) -> re_viewer_context::SpaceViewClassLayoutPriority {
        re_viewer_context::SpaceViewClassLayoutPriority::High
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

        let context = SpatialTopology::access(entity_db.store_id(), |topo| {
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
            let reprojectable_3d_entities =
                if primary_space.connection_to_parent.is_connected_pinhole() {
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

    fn on_frame_start(
        &self,
        ctx: &ViewerContext<'_>,
        state: &Self::State,
        ent_paths: &PerSystemEntities,
        entity_properties: &mut re_entity_db::EntityPropertyMap,
    ) {
        update_object_property_heuristics(
            ctx,
            ent_paths,
            entity_properties,
            &state.bounding_boxes.accumulated,
            SpatialSpaceViewKind::TwoD,
        );
    }

    fn spawn_heuristics(
        &self,
        ctx: &ViewerContext<'_>,
    ) -> re_viewer_context::SpaceViewSpawnHeuristics {
        re_tracing::profile_function!();

        let indicated_entities = default_visualized_entities_for_visualizer_kind(
            ctx,
            self.identifier(),
            SpatialSpaceViewKind::TwoD,
        );

        let image_dimensions =
            MaxImageDimensions::access(ctx.entity_db.store_id(), |image_dimensions| {
                image_dimensions.clone()
            })
            .unwrap_or_default();

        // Spawn a space view at each subspace that has any potential 2D content.
        // Note that visualizability filtering is all about being in the right subspace,
        // so we don't need to call the visualizers' filter functions here.
        SpatialTopology::access(ctx.entity_db.store_id(), |topo| SpaceViewSpawnHeuristics {
            recommended_space_views: topo
                .iter_subspaces()
                .flat_map(|subspace| {
                    if !subspace.supports_2d_content()
                        || subspace.entities.is_empty()
                        || indicated_entities.is_disjoint(&subspace.entities)
                    {
                        return Vec::new();
                    }

                    // Collect just the 2d-relevant entities in this subspace
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

                    let mut recommended_space_views = Vec::<RecommendedSpaceView>::new();

                    recommended_space_views_with_image_splits(
                        ctx,
                        &image_dimensions,
                        &recommended_root,
                        &relevant_entities,
                        &mut recommended_space_views,
                    );

                    recommended_space_views
                })
                .collect(),
        })
        .unwrap_or_default()
    }

    fn selection_ui(
        &self,
        ctx: &re_viewer_context::ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        space_origin: &EntityPath,
        _space_view_id: SpaceViewId,
        _root_entity_properties: &mut EntityProperties,
    ) {
        state.selection_ui(ctx, ui, space_origin, SpatialSpaceViewKind::TwoD);
    }

    fn ui(
        &self,
        ctx: &ViewerContext<'_>,
        ui: &mut egui::Ui,
        state: &mut Self::State,
        _root_entity_properties: &EntityProperties,
        query: &ViewQuery<'_>,
        system_output: re_viewer_context::SystemExecutionOutput,
    ) -> Result<(), SpaceViewSystemExecutionError> {
        re_tracing::profile_function!();

        state.bounding_boxes.update(&system_output.view_systems);
        state.scene_num_primitives = system_output
            .context_systems
            .get::<PrimitiveCounter>()?
            .num_primitives
            .load(std::sync::atomic::Ordering::Relaxed);

        crate::ui_2d::view_2d(ctx, ui, state, query, system_output)
    }
}

// Count the number of image entities with the given component exist that aren't
// children of other entities in the bucket.
fn count_non_nested_images_with_component(
    image_dimensions: &IntMap<EntityPath, ImageDimensions>,
    entity_bucket: &IntSet<EntityPath>,
    subtree: &EntityTree,
    component_name: &ComponentName,
) -> usize {
    if image_dimensions.contains_key(&subtree.path) {
        // bool true -> 1
        subtree.entity.components.contains_key(component_name) as usize
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
    image_dimensions: &IntMap<EntityPath, ImageDimensions>,
    entity_bucket: &IntSet<EntityPath>,
    subtree: &EntityTree,
    found_image_dimensions: &mut HashSet<[u64; 2]>,
) {
    if let Some(dimensions) = image_dimensions.get(&subtree.path) {
        // If we found an image entity, add its dimensions to the set.
        found_image_dimensions.insert([dimensions.height, dimensions.width]);
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

fn recommended_space_views_with_image_splits(
    ctx: &ViewerContext<'_>,
    image_dimensions: &IntMap<EntityPath, ImageDimensions>,
    recommended_root: &EntityPath,
    entities: &IntSet<EntityPath>,
    recommended: &mut Vec<RecommendedSpaceView>,
) {
    re_tracing::profile_function!();

    let tree = ctx.entity_db.tree();

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

    let image_count = count_non_nested_images_with_component(
        image_dimensions,
        entities,
        subtree,
        &Image::indicator().name(),
    );

    let depth_count = count_non_nested_images_with_component(
        image_dimensions,
        entities,
        subtree,
        &DepthImage::indicator().name(),
    );

    // If there are images of multiple dimensions, more than 1 image, or more than 1 depth image
    // then split the space.
    if found_image_dimensions.len() > 1 || image_count > 1 || depth_count > 1 {
        // Otherwise, split the space and recurse

        // If the root also had a visualizable entity, give it its own space.
        // TODO(jleibs): Maybe merge this entity into each child
        if entities.contains(recommended_root) {
            recommended.push(RecommendedSpaceView {
                root: recommended_root.clone(),
                query_filter: EntityPathFilter::single_entity_filter(recommended_root),
            });
        }

        // And then recurse into the children
        for child in subtree.children.values() {
            let sub_bucket: IntSet<_> = entities
                .iter()
                .filter(|e| e.starts_with(&child.path))
                .cloned()
                .collect();

            if !sub_bucket.is_empty() {
                recommended_space_views_with_image_splits(
                    ctx,
                    image_dimensions,
                    &child.path,
                    &sub_bucket,
                    recommended,
                );
            }
        }
    } else {
        // Otherwise we can use the space as it is
        recommended.push(RecommendedSpaceView {
            root: recommended_root.clone(),
            query_filter: EntityPathFilter::subtree_entity_filter(recommended_root),
        });
    }
}
