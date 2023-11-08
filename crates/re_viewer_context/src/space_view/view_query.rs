use std::collections::BTreeMap;

use itertools::Itertools;
use re_arrow_store::LatestAtQuery;
use re_data_store::{EntityPath, EntityProperties, TimeInt, Timeline};

use crate::{SpaceViewHighlights, SpaceViewId, ViewSystemName};

#[derive(Debug)]
pub struct DataResult {
    // TODO(jleibs): This should eventually become a more generalized (StoreView + EntityPath) reference to handle
    // multi-RRD or blueprint-static data references.
    pub entity_path: EntityPath,

    pub view_parts: Vec<ViewSystemName>,

    // TODO(jleibs): Eventually this goes away and becomes implicit as an override layer in the StoreView
    // The reason we store it here though is that context is part of the DataResult.
    pub resolved_properties: EntityProperties,
}

pub type PerSystemDataResults<'a> = BTreeMap<ViewSystemName, Vec<&'a DataResult>>;

pub struct ViewQuery<'s> {
    /// The id of the space in which context the query happens.
    pub space_view_id: SpaceViewId,

    /// The root of the space in which context the query happens.
    pub space_origin: &'s EntityPath,

    /// All queried entities.
    ///
    /// Contains also invisible objects, use `iter_entities` to iterate over visible ones.
    pub per_system_data_results: &'s PerSystemDataResults<'s>,

    /// The timeline we're on.
    pub timeline: Timeline,

    /// The time on the timeline we're currently at.
    pub latest_at: TimeInt,

    /// Hover/select highlighting information for this space view.
    ///
    /// TODO(andreas): This should be a [`crate::ViewContextSystem`] instead.
    pub highlights: &'s SpaceViewHighlights,
}

impl<'s> ViewQuery<'s> {
    /// Iter over all of the currently visible [`EntityPath`]s in the [`ViewQuery`].
    ///
    /// Also includes the corresponding [`EntityProperties`].
    pub fn iter_entities_and_properties_for_system(
        &self,
        system: ViewSystemName,
    ) -> impl Iterator<Item = (&EntityPath, &EntityProperties)> {
        self.per_system_data_results.get(&system).map_or(
            itertools::Either::Left(std::iter::empty()),
            |results| {
                itertools::Either::Right(
                    results
                        .iter()
                        .map(|data_result| {
                            (&data_result.entity_path, &data_result.resolved_properties)
                        })
                        .filter(|(_, props)| props.visible),
                )
            },
        )
    }

    /// Iterates over all [`DataResult`]s of the [`ViewQuery`].
    pub fn iter_data_results(&self) -> impl Iterator<Item = &DataResult> + '_ {
        self.per_system_data_results
            .values()
            .flat_map(|data_results| data_results.iter().copied())
    }

    /// Iterates over all entities of the [`ViewQuery`].
    pub fn iter_entities(&self) -> impl Iterator<Item = &EntityPath> + '_ {
        self.iter_data_results()
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
