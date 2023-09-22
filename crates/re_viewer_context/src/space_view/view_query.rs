use itertools::Itertools;
use re_arrow_store::LatestAtQuery;
use re_data_store::{EntityPath, EntityProperties, EntityPropertyMap, TimeInt, Timeline};

use crate::{PerSystemEntities, SpaceViewHighlights, SpaceViewId, ViewSystemName};

pub struct ViewQuery<'s> {
    /// The id of the space in which context the query happens.
    pub space_view_id: SpaceViewId,

    /// The root of the space in which context the query happens.
    pub space_origin: &'s EntityPath,

    /// All queried entities.
    ///
    /// Contains also invisible objects, use `iter_entities` to iterate over visible ones.
    pub per_system_entities: &'s PerSystemEntities,

    /// The timeline we're on.
    pub timeline: Timeline,

    /// The time on the timeline we're currently at.
    pub latest_at: TimeInt,

    /// The entity properties for all queried entities.
    /// TODO(jleibs, wumpf): This will be replaced by blueprint queries.
    pub entity_props_map: &'s EntityPropertyMap,

    /// Hover/select highlighting information for this space view.
    ///
    /// TODO(andreas): This should be a [`crate::ViewContextSystem`] instead.
    pub highlights: &'s SpaceViewHighlights,
}

impl<'s> ViewQuery<'s> {
    /// Iter over all of the currently visible [`EntityPath`]s in the [`ViewQuery`].
    ///
    /// Also includes the corresponding [`EntityProperties`].
    pub fn iter_entities_for_system(
        &self,
        system: ViewSystemName,
    ) -> impl Iterator<Item = (&EntityPath, EntityProperties)> {
        self.per_system_entities.get(&system).map_or(
            itertools::Either::Left(std::iter::empty()),
            |entities| {
                itertools::Either::Right(
                    entities
                        .iter()
                        .map(|entity_path| (entity_path, self.entity_props_map.get(entity_path)))
                        .filter(|(_entity_path, props)| props.visible),
                )
            },
        )
    }

    /// Iterates over all entities of the [`ViewQuery`].
    pub fn iter_entities(&self) -> impl Iterator<Item = &EntityPath> + '_ {
        self.per_system_entities
            .values()
            .flat_map(|entities| entities.iter())
            .unique()
    }

    pub fn latest_at_query(&self) -> LatestAtQuery {
        LatestAtQuery {
            timeline: self.timeline,
            at: self.latest_at,
        }
    }
}
