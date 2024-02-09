use ahash::HashMap;
use itertools::Itertools;
use nohash_hasher::IntSet;

use re_entity_db::{EntityProperties, EntityTree};
use re_log_types::{EntityPath, EntityPathFilter};
use re_tracing::profile_scope;
use re_types::{
    archetypes::{DepthImage, Image},
    components::TensorData,
    Archetype, ComponentName,
};
use re_viewer_context::{
    ApplicableEntities, IdentifiedViewSystem as _, PerSystemEntities, RecommendedSpaceView,
    SpaceViewClass, SpaceViewClassRegistryError, SpaceViewId, SpaceViewSpawnHeuristics,
    SpaceViewSystemExecutionError, ViewQuery, ViewerContext, VisualizableFilterContext,
};

use crate::{
    contexts::{register_spatial_contexts, PrimitiveCounter},
    heuristics::{
        default_visualized_entities_for_visualizer_kind, update_object_property_heuristics,
    },
    spatial_topology::{SpatialTopology, SubSpace, SubSpaceDimensionality},
    ui::SpatialSpaceViewState,
    view_kind::SpatialSpaceViewKind,
    visualizers::{register_2d_spatial_visualizers, ImageVisualizer},
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
            match primary_space.dimensionality {
                SubSpaceDimensionality::Unknown => VisualizableFilterContext2D {
                    entities_in_main_2d_space: primary_space.entities.clone(),
                    reprojectable_3d_entities: Default::default(),
                },

                SubSpaceDimensionality::TwoD => {
                    // All entities in the 2d space are visualizable + the parent space if it is connected via a pinhole.
                    // For the moment we don't allow going down pinholes again.
                    let reprojected_3d_entities = primary_space
                        .parent_space
                        .and_then(|parent_space_origin| {
                            let is_connected_pinhole = topo
                                .subspace_for_subspace_origin(parent_space_origin)
                                .and_then(|parent_space| {
                                    parent_space
                                        .child_spaces
                                        .get(&primary_space.origin)
                                        .map(|connection| connection.is_connected_pinhole())
                                })
                                .unwrap_or(false);

                            if is_connected_pinhole {
                                topo.subspace_for_subspace_origin(parent_space_origin)
                                    .map(|parent_space| parent_space.entities.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default();

                    VisualizableFilterContext2D {
                        entities_in_main_2d_space: primary_space.entities.clone(),
                        reprojectable_3d_entities: reprojected_3d_entities,
                    }
                }

                SubSpaceDimensionality::ThreeD => {
                    // If this is 3D space, only display the origin entity itself.
                    // Everything else we have to assume requires some form of transformation.
                    VisualizableFilterContext2D {
                        entities_in_main_2d_space: std::iter::once(space_origin.clone()).collect(),
                        reprojectable_3d_entities: Default::default(),
                    }
                }
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

        let image_entities_fallback = ApplicableEntities::default();
        let image_entities = ctx
            .applicable_entities_per_visualizer
            .get(&ImageVisualizer::identifier())
            .unwrap_or(&image_entities_fallback);

        // Spawn a space view at each subspace that has any potential 2D content.
        // Note that visualizability filtering is all about being in the right subspace,
        // so we don't need to call the visualizers' filter functions here.
        let mut heuristics =
            SpatialTopology::access(ctx.entity_db.store_id(), |topo| SpaceViewSpawnHeuristics {
                recommended_space_views: topo
                    .iter_subspaces()
                    .flat_map(|subspace| {
                        if subspace.dimensionality == SubSpaceDimensionality::ThreeD
                            || subspace.entities.is_empty()
                            || indicated_entities.is_disjoint(&subspace.entities)
                        {
                            return Vec::new();
                        }

                        let mut recommended_space_views = Vec::<RecommendedSpaceView>::new();

                        for bucket_entities in
                            bucket_images_in_subspace(ctx, subspace, image_entities)
                        {
                            add_recommended_space_views_for_bucket(
                                ctx,
                                &bucket_entities,
                                &mut recommended_space_views,
                            );
                        }

                        // If we only recommended 1 space view from the bucketing, we're better off using the
                        // root of the subspace, below. If there were multiple subspaces, keep them, even if
                        // they may be redundant with the root space.
                        if recommended_space_views.len() == 1 {
                            recommended_space_views.clear();
                        }

                        // If this is explicitly a 2D subspace (such as from a pinhole), or there were no
                        // other image-bucketed recommendations, create a space at the root of the subspace.
                        if subspace.dimensionality == SubSpaceDimensionality::TwoD
                            || recommended_space_views.is_empty()
                        {
                            recommended_space_views.push(RecommendedSpaceView {
                                root: subspace.origin.clone(),
                                query_filter: EntityPathFilter::subtree_entity_filter(
                                    &subspace.origin,
                                ),
                            });
                        }

                        recommended_space_views
                    })
                    .collect(),
            })
            .unwrap_or_default();

        // Find all entities that are not yet covered by the recommended space views and create a recommended
        // space-view for each one at that specific entity path.
        // TODO(jleibs): This is expensive. Would be great to track this as we build up the covering instead.
        {
            profile_scope!("space_view_2d: find uncovered entities");
            let remaining_entities = indicated_entities
                .iter()
                .filter(|entity| {
                    heuristics
                        .recommended_space_views
                        .iter()
                        .all(|r| !r.query_filter.is_included(entity))
                })
                .collect::<Vec<_>>();

            for entity in remaining_entities {
                heuristics
                    .recommended_space_views
                    .push(RecommendedSpaceView {
                        root: entity.clone(),
                        query_filter: EntityPathFilter::single_entity_filter(entity),
                    });
            }
        }

        heuristics
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

fn count_non_nested_entities_with_component(
    entity_bucket: &IntSet<EntityPath>,
    subtree: &EntityTree,
    component_name: &ComponentName,
) -> usize {
    if entity_bucket.contains(&subtree.path) {
        // bool true -> 1
        subtree.entity.components.contains_key(component_name) as usize
    } else if !entity_bucket
        .iter()
        .any(|e| e.is_descendant_of(&subtree.path))
    {
        0
    } else {
        subtree
            .children
            .values()
            .map(|child| {
                count_non_nested_entities_with_component(entity_bucket, child, component_name)
            })
            .sum()
    }
}

fn add_recommended_space_views_for_bucket(
    ctx: &ViewerContext<'_>,
    entity_bucket: &IntSet<EntityPath>,
    recommended: &mut Vec<RecommendedSpaceView>,
) {
    // TODO(jleibs): Converting entity_bucket to a Trie would probably make some of this easier.
    let tree = ctx.entity_db.tree();

    // Find the common ancestor of the bucket
    let root = EntityPath::common_ancestor_of(entity_bucket.iter());

    // If the root of this bucket contains an image itself, this means the rest of the content
    // is nested under some kind of 2d-visualizable thing. We expect the user meant to create
    // a layered 2d space.
    if entity_bucket.contains(&root) {
        recommended.push(RecommendedSpaceView {
            root: root.clone(),
            query_filter: EntityPathFilter::subtree_entity_filter(&root),
        });
        return;
    }

    // Alternatively we want to split this bucket into a group for each child-space.
    let Some(subtree) = tree.subtree(&root) else {
        if cfg!(debug_assertions) {
            re_log::warn_once!("Ancestor of entity not found in entity tree.");
        }
        return;
    };

    let image_count = count_non_nested_entities_with_component(
        entity_bucket,
        subtree,
        &Image::indicator().name(),
    );

    let depth_count = count_non_nested_entities_with_component(
        entity_bucket,
        subtree,
        &DepthImage::indicator().name(),
    );

    // If there's no more than 1 image and 1 depth image at any of the top-level of the sub-buckets, we can still
    // recommend the root.
    if image_count <= 1 && depth_count <= 1 {
        recommended.push(RecommendedSpaceView {
            root: root.clone(),
            query_filter: EntityPathFilter::subtree_entity_filter(&root),
        });
        return;
    }

    // Otherwise, split the space and recurse
    for child in subtree.children.values() {
        let sub_bucket: IntSet<_> = entity_bucket
            .iter()
            .filter(|e| e.starts_with(&child.path))
            .cloned()
            .collect();

        if !sub_bucket.is_empty() {
            add_recommended_space_views_for_bucket(ctx, &sub_bucket, recommended);
        }
    }
}

/// Groups all images in the subspace by size and draw order.
fn bucket_images_in_subspace(
    ctx: &ViewerContext<'_>,
    subspace: &SubSpace,
    image_entities: &ApplicableEntities,
) -> Vec<IntSet<EntityPath>> {
    re_tracing::profile_function!();

    let store = ctx.entity_db.store();

    let image_entities = subspace
        .entities
        .iter()
        .filter(|e| image_entities.contains(e))
        .collect_vec();

    if image_entities.len() <= 1 {
        // Very common case, early out before we get into the more expensive query code.
        return image_entities
            .into_iter()
            .map(|e| std::iter::once(e.clone()).collect())
            .collect();
    }

    let mut images_by_bucket = HashMap::<(u64, u64), IntSet<EntityPath>>::default();
    for image_entity in image_entities {
        // TODO(andreas): We really don't want to do a latest at query here since this means the heuristic can have different results depending on the
        //                current query, but for this we'd have to store the max-size over time somewhere using another store subscriber (?).
        if let Some(tensor) =
            store.query_latest_component::<TensorData>(image_entity, &ctx.current_query())
        {
            if let Some([height, width, _]) = tensor.image_height_width_channels() {
                // 1D tensors are typically handled by tensor or bar chart views and make generally for poor image buckets!
                if height > 1 && width > 1 {
                    images_by_bucket
                        .entry((height, width))
                        .or_default()
                        .insert(image_entity.clone());
                }
            }
        }
    }

    images_by_bucket.drain().map(|(_, v)| v).collect()
}
