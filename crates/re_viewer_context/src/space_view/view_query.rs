use std::collections::BTreeMap;

use itertools::Itertools;
use nohash_hasher::IntMap;
use once_cell::sync::Lazy;

use re_data_store::LatestAtQuery;
use re_entity_db::{EntityPath, EntityProperties, EntityPropertiesComponent, TimeInt, Timeline};
use re_log_types::{DataCell, DataRow, RowId, StoreKind};
use re_types::{ComponentName, Loggable};
use smallvec::SmallVec;

use crate::{
    blueprint_timepoint_for_writes, SpaceViewHighlights, SpaceViewId, SystemCommand,
    SystemCommandSender as _, ViewSystemIdentifier, ViewerContext,
};

#[derive(Clone, Debug, PartialEq)]
pub struct PropertyOverrides {
    /// The accumulated properties (including any hierarchical flattening) to apply.
    // TODO(jleibs): Eventually this goes away and becomes implicit as an override layer in the StoreView.
    // For now, bundling this here acts as a good proxy for that future data-override mechanism.
    pub accumulated_properties: EntityProperties,

    /// The individual property set in this `DataResult`, if any.
    pub individual_properties: Option<EntityProperties>,

    /// The recursive property set in this `DataResult`, if any.
    pub recursive_properties: Option<EntityProperties>,

    /// An alternative store and entity path to use for the specified component.
    ///
    /// These are resolved overrides, i.e. the result of recursive override propagation + individual overrides.
    // NOTE: StoreKind is easier to work with than a `StoreId`` or full `DataStore` but
    // might still be ambiguous when we have multiple stores active at a time.
    // TODO(jleibs): Consider something like `tinymap` for this.
    // TODO(andreas): Should be a `Cow` to not do as many clones.
    // TODO(andreas): Track recursive vs resolved (== individual + recursive) overrides.
    //                  Recursive here meaning inherited + own recursive, i.e. not just what's on the path.
    //                  What is logged on *this* entity can be inferred from walking up the tree.
    pub resolved_component_overrides: IntMap<ComponentName, (StoreKind, EntityPath)>,

    /// `EntityPath` in the Blueprint store where updated overrides should be written back
    /// for properties that apply recursively.
    pub recursive_override_path: EntityPath,

    /// `EntityPath` in the Blueprint store where updated overrides should be written back
    /// for properties that apply to the individual entity only.
    pub individual_override_path: EntityPath,
}

pub type SmallVisualizerSet = SmallVec<[ViewSystemIdentifier; 4]>;

/// This is the primary mechanism through which data is passed to a `SpaceView`.
///
/// It contains everything necessary to properly use this data in the context of the
/// `ViewSystem`s that it is a part of.
///
/// In the future `accumulated_properties` will be replaced by a `StoreView` that contains
/// the relevant data overrides for the given query.
#[derive(Clone, Debug, PartialEq)]
pub struct DataResult {
    /// Where to retrieve the data from.
    // TODO(jleibs): This should eventually become a more generalized (StoreView + EntityPath) reference to handle
    // multi-RRD or blueprint-static data references.
    pub entity_path: EntityPath,

    /// Which `ViewSystems`s to pass the `DataResult` to.
    pub visualizers: SmallVisualizerSet,

    /// If true, this path is not actually included in the query results and is just here
    /// because of a common prefix.
    ///
    /// If this is true, `visualizers` must be empty.
    pub tree_prefix_only: bool,

    /// The accumulated property overrides for this `DataResult`.
    pub property_overrides: Option<PropertyOverrides>,
}

static DEFAULT_PROPS: Lazy<EntityProperties> = Lazy::<EntityProperties>::new(Default::default);

impl DataResult {
    pub const INDIVIDUAL_OVERRIDES_PREFIX: &'static str = "individual_overrides";
    pub const RECURSIVE_OVERRIDES_PREFIX: &'static str = "recursive_overrides";

    #[inline]
    pub fn recursive_override_path(&self) -> Option<&EntityPath> {
        self.property_overrides
            .as_ref()
            .map(|p| &p.recursive_override_path)
    }

    #[inline]
    pub fn individual_override_path(&self) -> Option<&EntityPath> {
        self.property_overrides
            .as_ref()
            .map(|p| &p.individual_override_path)
    }

    /// Write the [`EntityProperties`] for this result back to the Blueprint store on the recursive override.
    ///
    /// Setting `new_recursive_props` to `None` will always clear the override.
    /// Otherwise, writes only if the recursive properties aren't already the same value.
    /// (does *not* take into account what the accumulated properties are which are a combination of recursive and individual overwrites)
    pub fn save_recursive_override(
        &self,
        ctx: &ViewerContext<'_>,
        new_recursive_props: Option<EntityProperties>,
    ) {
        self.save_override_internal(
            ctx,
            new_recursive_props,
            self.recursive_override_path(),
            self.recursive_properties(),
        );
    }

    /// Write the [`EntityProperties`] for this result back to the Blueprint store on the individual override.
    ///
    /// Setting `new_individual_props` to `None` will always clear the override.
    /// Otherwise, writes only if the individual properties aren't already the same value.
    /// (does *not* take into account what the accumulated properties are which are a combination of recursive and individual overwrites)
    pub fn save_individual_override(
        &self,
        new_individual_props: Option<EntityProperties>,
        ctx: &ViewerContext<'_>,
    ) {
        self.save_override_internal(
            ctx,
            new_individual_props,
            self.individual_override_path(),
            self.individual_properties(),
        );
    }

    fn save_override_internal(
        &self,
        ctx: &ViewerContext<'_>,
        new_individual_props: Option<EntityProperties>,
        override_path: Option<&EntityPath>,
        properties: Option<&EntityProperties>,
    ) {
        // TODO(jleibs): Make it impossible for this to happen with different type structure
        // This should never happen unless we're doing something with a partially processed
        // query.
        let Some(override_path) = override_path else {
            re_log::warn!(
                "Tried to save override for {:?} but it has no override path",
                self.entity_path
            );
            return;
        };

        let cell = match new_individual_props {
            None => {
                re_log::debug!("Clearing {:?}", override_path);

                Some(DataCell::from_arrow_empty(
                    EntityPropertiesComponent::name(),
                    EntityPropertiesComponent::arrow_datatype(),
                ))
            }
            Some(props) => {
                // A value of `None` in the data store means "use the default value", so if
                // the properties are `None`, we only must save if `props` is different
                // from the default.
                if props.has_edits(properties.unwrap_or(&DEFAULT_PROPS)) {
                    re_log::debug!("Overriding {:?} with {:?}", override_path, props);

                    let component = EntityPropertiesComponent(props);

                    Some(DataCell::from([component]))
                } else {
                    None
                }
            }
        };

        if let Some(cell) = cell {
            let timepoint = blueprint_timepoint_for_writes();

            let row =
                DataRow::from_cells1_sized(RowId::new(), override_path.clone(), timepoint, 1, cell)
                    .unwrap();

            ctx.command_sender
                .send_system(SystemCommand::UpdateBlueprint(
                    ctx.store_context.blueprint.store_id().clone(),
                    vec![row],
                ));
        }
    }

    #[inline]
    pub fn accumulated_properties(&self) -> &EntityProperties {
        // TODO(jleibs): Make it impossible for this to happen with different type structure
        // This should never happen unless we're doing something with a partially processed
        // query.
        let Some(property_overrides) = &self.property_overrides else {
            re_log::warn!(
                "Tried to get accumulated properties for {:?} but it has no property overrides",
                self.entity_path
            );
            return &DEFAULT_PROPS;
        };

        &property_overrides.accumulated_properties
    }

    #[inline]
    pub fn recursive_properties(&self) -> Option<&EntityProperties> {
        self.property_overrides
            .as_ref()
            .and_then(|p| p.recursive_properties.as_ref())
    }

    #[inline]
    pub fn individual_properties(&self) -> Option<&EntityProperties> {
        self.property_overrides
            .as_ref()
            .and_then(|p| p.individual_properties.as_ref())
    }

    pub fn lookup_override<C: re_types::Component>(&self, ctx: &ViewerContext<'_>) -> Option<C> {
        self.property_overrides
            .as_ref()
            .and_then(|p| p.resolved_component_overrides.get(&C::name()))
            .and_then(|(store_kind, path)| match store_kind {
                StoreKind::Blueprint => ctx
                    .store_context
                    .blueprint
                    .store()
                    .query_latest_component::<C>(path, ctx.blueprint_query),
                StoreKind::Recording => ctx
                    .entity_db
                    .store()
                    .query_latest_component::<C>(path, &ctx.current_query()),
            })
            .map(|c| c.value)
    }

    #[inline]
    pub fn lookup_override_or_default<C: re_types::Component + Default>(
        &self,
        ctx: &ViewerContext<'_>,
    ) -> C {
        self.lookup_override(ctx).unwrap_or_default()
    }

    /// Shorthand for checking for visibility on data overrides.
    ///
    /// Note that this won't check if the data store has visibility logged.
    // TODO(andreas): Should this be possible?
    // TODO(andreas): Should the result be cached, this might be a very common operation?
    #[inline]
    pub fn is_visible(&self, ctx: &ViewerContext<'_>) -> bool {
        self.lookup_override_or_default::<re_types::blueprint::components::Visible>(ctx)
            .0
    }
}

pub type PerSystemDataResults<'a> = BTreeMap<ViewSystemIdentifier, Vec<&'a DataResult>>;

pub struct ViewQuery<'s> {
    /// The id of the space in which context the query happens.
    pub space_view_id: SpaceViewId,

    /// The root of the space in which context the query happens.
    pub space_origin: &'s EntityPath,

    /// All queried [`DataResult`]s.
    ///
    /// Contains also invisible objects, use `iter_entities` to iterate over visible ones.
    pub per_system_data_results: PerSystemDataResults<'s>,

    /// The timeline we're on.
    pub timeline: Timeline,

    /// The time on the timeline we're currently at.
    pub latest_at: TimeInt,

    /// Hover/select highlighting information for this space view.
    ///
    /// TODO(andreas): This should be the result of a [`crate::ViewContextSystem`] instead?
    pub highlights: SpaceViewHighlights,
}

impl<'s> ViewQuery<'s> {
    /// Iter over all of the currently visible [`DataResult`]s for a given `ViewSystem`
    pub fn iter_visible_data_results<'a>(
        &'a self,
        ctx: &'a ViewerContext<'a>,
        system: ViewSystemIdentifier,
    ) -> impl Iterator<Item = &DataResult>
    where
        's: 'a,
    {
        self.per_system_data_results.get(&system).map_or(
            itertools::Either::Left(std::iter::empty()),
            |results| {
                itertools::Either::Right(
                    results
                        .iter()
                        .filter(|result| result.is_visible(ctx))
                        .copied(),
                )
            },
        )
    }

    /// Iterates over all [`DataResult`]s of the [`ViewQuery`].
    pub fn iter_all_data_results(&self) -> impl Iterator<Item = &DataResult> + '_ {
        self.per_system_data_results
            .values()
            .flat_map(|data_results| data_results.iter().copied())
    }

    /// Iterates over all entities of the [`ViewQuery`].
    pub fn iter_all_entities(&self) -> impl Iterator<Item = &EntityPath> + '_ {
        self.iter_all_data_results()
            .map(|data_result| &data_result.entity_path)
            .unique()
    }

    pub fn latest_at_query(&self) -> LatestAtQuery {
        LatestAtQuery {
            timeline: self.timeline,
            at: self.latest_at,
        }
    }
}
