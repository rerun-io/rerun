use ahash::HashMap;
use itertools::Itertools;
use nohash_hasher::IntSet;

use re_entity_db::EntityProperties;
use re_log_types::{EntityPath, EntityPathFilter};
use re_tracing::profile_scope;
use re_types::components::TensorData;
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
        let mut heuristics = SpatialTopology::access(ctx.entity_db.store_id(), |topo| {
            SpaceViewSpawnHeuristics {
                recommended_space_views: topo
                    .iter_subspaces()
                    .flat_map(|subspace| {
                        if subspace.dimensionality == SubSpaceDimensionality::ThreeD
                            || subspace.entities.is_empty()
                            || indicated_entities.is_disjoint(&subspace.entities)
                        {
                            return Vec::new();
                        }

                        let images_by_bucket =
                            bucket_images_in_subspace(ctx, subspace, image_entities);

                        if images_by_bucket.len() <= 1 {
                            // If there's no or only a single image bucket, use the whole subspace to capture all the non-image entities!
                            vec![RecommendedSpaceView {
                                root: subspace.origin.clone(),
                                query_filter: EntityPathFilter::subtree_entity_filter(
                                    &subspace.origin,
                                ),
                            }]
                        } else {
                            #[allow(clippy::iter_kv_map)] // Not doing `values()` saves a path copy!
                            images_by_bucket
                                .into_iter()
                                .map(|(_, entity_bucket)| {
                                    // Pick a shared parent as origin.
                                    // Mostly because it looks nicer in the ui.
                                    let root = entity_bucket.iter().skip(1).fold(
                                        entity_bucket
                                            .first()
                                            .unwrap_or(&EntityPath::root())
                                            .clone(),
                                        |acc, e| acc.common_ancestor(e),
                                    );

                                    let mut query_filter = EntityPathFilter::default();
                                    for image in &entity_bucket {
                                        // This might lead to overlapping subtrees and break the same image size bucketing again.
                                        // We just take that risk, the heuristic doesn't need to be perfect.
                                        query_filter.add_subtree(image.clone());
                                    }

                                    RecommendedSpaceView { root, query_filter }
                                })
                                .collect()
                        }
                    })
                    .collect(),
            }
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

/// Groups all images in the subspace by size and draw order.
fn bucket_images_in_subspace(
    ctx: &ViewerContext<'_>,
    subspace: &SubSpace,
    image_entities: &ApplicableEntities,
) -> HashMap<(u64, u64), Vec<EntityPath>> {
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
            .map(|e| ((0, 0), vec![e.clone()]))
            .collect();
    }

    let mut images_by_bucket = HashMap::<(u64, u64), Vec<EntityPath>>::default();
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
                        .push(image_entity.clone());
                }
            }
        }
    }

    images_by_bucket
}
