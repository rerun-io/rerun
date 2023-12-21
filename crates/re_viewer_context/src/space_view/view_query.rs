use std::collections::BTreeMap;

use itertools::Itertools;
use once_cell::sync::Lazy;
use re_data_store::LatestAtQuery;
use re_entity_db::{EntityPath, EntityProperties, EntityPropertiesComponent, TimeInt, Timeline};
use re_log_types::{DataCell, DataRow, RowId};
use re_types::Loggable;
use smallvec::SmallVec;

use crate::{
    blueprint_timepoint, SpaceViewHighlights, SpaceViewId, SystemCommand, SystemCommandSender as _,
    ViewSystemIdentifier, ViewerContext,
};

#[derive(Clone, Debug, PartialEq)]
pub struct PropertyOverrides {
    /// The accumulated properties (including any hierarchical flattening) to apply.
    // TODO(jleibs): Eventually this goes away and becomes implicit as an override layer in the StoreView.
    // For now, bundling this here acts as a good proxy for that future data-override mechanism.
    pub accumulated_properties: EntityProperties,

    /// The individual property set in this `DataResult`, if any.
    pub individual_properties: Option<EntityProperties>,

    /// `EntityPath` in the Blueprint store where updated overrides should be written back.
    pub override_path: EntityPath,
}

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
    pub visualizers: SmallVec<[ViewSystemIdentifier; 4]>,

    /// This DataResult represents a group
    // TODO(jleibs): Maybe make this an enum instead?
    pub is_group: bool,

    // This result was actually in the query results, not just a group that
    // exists due to a common prefix.
    pub direct_included: bool,

    /// The accumulated property overrides for this `DataResult`.
    pub property_overrides: Option<PropertyOverrides>,
}

static DEFAULT_PROPS: Lazy<EntityProperties> = Lazy::<EntityProperties>::new(Default::default);

impl DataResult {
    pub const INDIVIDUAL_OVERRIDES_PREFIX: &'static str = "individual_overrides";
    pub const RECURSIVE_OVERRIDES_PREFIX: &'static str = "recursive_overrides";

    #[inline]
    pub fn override_path(&self) -> Option<&EntityPath> {
        self.property_overrides.as_ref().map(|p| &p.override_path)
    }

    /// Write the [`EntityProperties`] for this result back to the Blueprint store.
    pub fn save_override(&self, props: Option<EntityProperties>, ctx: &ViewerContext<'_>) {
        // TODO(jleibs): Make it impossible for this to happen with different type structure
        // This should never happen unless we're doing something with a partially processed
        // query.
        let Some(override_path) = self.override_path() else {
            re_log::warn!(
                "Tried to save override for {:?} but it has no override path",
                self.entity_path
            );
            return;
        };

        // This should never happen if the above didn't return early.
        let Some(property_overrides) = &self.property_overrides else {
            return;
        };

        let cell = match props {
            None => {
                re_log::debug!("Clearing {:?}", override_path);

                Some(DataCell::from_arrow_empty(
                    EntityPropertiesComponent::name(),
                    EntityPropertiesComponent::arrow_datatype(),
                ))
            }
            Some(props) => {
                // A value of `None` in the data store means "use the default value", so if
                // `self.individual_properties` is `None`, we only must save if `props` is different
                // from the default.
                if props.has_edits(
                    property_overrides
                        .individual_properties
                        .as_ref()
                        .unwrap_or(&EntityProperties::default()),
                ) {
                    re_log::debug!("Overriding {:?} with {:?}", override_path, props);

                    let component = EntityPropertiesComponent(props);

                    Some(DataCell::from([component]))
                } else {
                    None
                }
            }
        };

        let Some(cell) = cell else {
            return;
        };

        let timepoint = blueprint_timepoint();

        let row =
            DataRow::from_cells1_sized(RowId::new(), override_path.clone(), timepoint, 1, cell)
                .unwrap();

        ctx.command_sender
            .send_system(SystemCommand::UpdateBlueprint(
                ctx.store_context.blueprint.store_id().clone(),
                vec![row],
            ));
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
    pub fn individual_properties(&self) -> Option<&EntityProperties> {
        self.property_overrides
            .as_ref()
            .and_then(|p| p.individual_properties.as_ref())
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
    pub fn iter_visible_data_results(
        &self,
        system: ViewSystemIdentifier,
    ) -> impl Iterator<Item = &DataResult> {
        self.per_system_data_results.get(&system).map_or(
            itertools::Either::Left(std::iter::empty()),
            |results| {
                itertools::Either::Right(
                    results
                        .iter()
                        .filter(|result| result.accumulated_properties().visible)
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
