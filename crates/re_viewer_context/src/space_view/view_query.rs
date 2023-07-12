use nohash_hasher::IntSet;

use re_data_store::{EntityPath, EntityProperties, EntityPropertyMap, TimeInt, Timeline};

use crate::SpaceViewHighlights;

pub struct ViewQuery<'s> {
    /// The root of the space in which context the query happens.
    pub space_origin: &'s EntityPath,

    /// All queried entities.
    ///
    /// Contains also invisible objects, use `iter_entities` to iterate over visible ones.
    pub entity_paths: &'s IntSet<EntityPath>,

    /// The timeline we're on.
    pub timeline: Timeline,

    /// The time on the timeline we're currently at.
    pub latest_at: TimeInt,

    /// The entity properties for all queried entities.
    /// TODO(jleibs/wumpf): This will be replaced by blueprint queries.
    pub entity_props_map: &'s EntityPropertyMap,

    /// Hover/select highlighting information for this space view.
    pub highlights: &'s SpaceViewHighlights,
}

impl<'s> ViewQuery<'s> {
    /// Iter over all of the currently visible [`EntityPath`]s in the [`ViewQuery`].
    ///
    /// Also includes the corresponding [`EntityProperties`].
    pub fn iter_entities(&self) -> impl Iterator<Item = (&EntityPath, EntityProperties)> {
        self.entity_paths
            .iter()
            .map(|entity_path| (entity_path, self.entity_props_map.get(entity_path)))
            .filter(|(_entity_path, props)| props.visible)
    }
}
