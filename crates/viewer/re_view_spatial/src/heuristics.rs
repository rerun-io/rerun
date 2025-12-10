use nohash_hasher::IntSet;
use re_log_types::EntityPath;
use re_sdk_types::ViewClassIdentifier;
use re_viewer_context::{ViewClass as _, ViewerContext};

use crate::view_kind::SpatialViewKind;
use crate::visualizers::SpatialViewVisualizerData;
use crate::{SpatialView2D, SpatialView3D};

pub struct IndicatedVisualizableEntities {
    /// All entities for which a visualizer of the given kind would be picked.
    ///
    /// I.e. all entities for which at least one visualizer of the specified kind is "maybe visualizable"
    /// *and* has a relevant archetype.
    /// (we can't reason with "visualizable" because that can be influenced by view properties like its origin)
    pub indicated_entities: IntSet<EntityPath>,

    /// Entity paths we know will not work within this visualizer.
    ///
    /// Right now this excludes things the 2D view would indicate in 3D, and vice versa.
    pub excluded_entities: Vec<EntityPath>,
}

impl IndicatedVisualizableEntities {
    pub fn new(
        ctx: &ViewerContext<'_>,
        view_class_identifier: ViewClassIdentifier,
        visualizer_kind: SpatialViewKind,
        include_entity: &dyn Fn(&EntityPath) -> bool,
        indicate_extra: impl FnOnce(&mut IntSet<EntityPath>),
    ) -> Self {
        let mut indicated_entities = default_visualized_entities_for_visualizer_kind(
            ctx,
            view_class_identifier,
            visualizer_kind,
            include_entity,
        );

        indicate_extra(&mut indicated_entities);

        let excluded_entities = default_excluded_entities_for_visualizer_kind(
            ctx,
            visualizer_kind,
            &indicated_entities,
        );

        Self {
            indicated_entities,
            excluded_entities,
        }
    }
}

fn default_visualized_entities_for_visualizer_kind(
    ctx: &ViewerContext<'_>,
    view_class_identifier: ViewClassIdentifier,
    visualizer_kind: SpatialViewKind,
    include_entity: &dyn Fn(&EntityPath) -> bool,
) -> IntSet<EntityPath> {
    re_tracing::profile_function!();

    ctx.view_class_registry()
        .new_visualizer_collection(view_class_identifier)
        .iter_with_identifiers()
        .filter_map(|(id, visualizer)| {
            let data = visualizer
                .data()?
                .downcast_ref::<SpatialViewVisualizerData>()?;

            if data.preferred_view_kind == Some(visualizer_kind) {
                let indicator_matching = ctx.indicated_entities_per_visualizer.get(&id)?;
                let visualizable = ctx.visualizable_entities_per_visualizer.get(&id)?;
                Some(
                    indicator_matching
                        .iter()
                        .filter(|e| visualizable.contains_key(e)),
                )
            } else {
                None
            }
        })
        .flatten()
        .filter(|e| include_entity(e))
        .cloned()
        .collect()
}

fn default_excluded_entities_for_visualizer_kind(
    ctx: &ViewerContext<'_>,
    visualizer_kind: SpatialViewKind,
    included_entities: &IntSet<EntityPath>,
) -> Vec<EntityPath> {
    let exclude_kind = match visualizer_kind {
        SpatialViewKind::TwoD => SpatialViewKind::ThreeD,
        SpatialViewKind::ThreeD => SpatialViewKind::TwoD,
    };

    let included_ancestors = included_entities
        .iter()
        .flat_map(|e| {
            let mut e = Some(e.clone());

            std::iter::from_fn(move || {
                let item = e.take();

                e = item.as_ref().and_then(|e| e.parent());

                item
            })
        })
        .collect::<IntSet<_>>();

    ctx.view_class_registry()
        .new_visualizer_collection(match exclude_kind {
            SpatialViewKind::TwoD => SpatialView2D::identifier(),
            SpatialViewKind::ThreeD => SpatialView3D::identifier(),
        })
        .iter_with_identifiers()
        .filter_map(|(id, visualizer)| {
            let data = visualizer
                .data()?
                .downcast_ref::<SpatialViewVisualizerData>()?;

            if data
                .preferred_view_kind
                .is_some_and(|vk| vk != visualizer_kind)
            {
                let indicator_matching = ctx.indicated_entities_per_visualizer.get(&id)?;
                Some(indicator_matching.iter())
            } else {
                None
            }
        })
        .flatten()
        // Don't exclude if we want to include one of it's descendants.
        .filter(|e| !included_ancestors.contains(e))
        // Find the highest ancestor we can use for this exclusion.
        .filter_map(|e| {
            let mut highest = None;
            let mut current = e.clone();

            loop {
                let Some(p) = current.parent() else {
                    if highest.is_none() {
                        highest = Some(current.clone());
                    }

                    break;
                };

                // If this is a child of an included entity always include it.
                if included_entities.contains(&p) {
                    return None;
                }

                if highest.is_none() && included_ancestors.contains(&p) {
                    highest = Some(current.clone());
                }

                current = p.clone();
            }

            highest
        })
        .collect()
}
