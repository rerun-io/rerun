use arrow2::datatypes::DataType;
use re_data_store::DataStore;
use re_log_types::{EntityPath, EntityPathHash, RowId, TimeInt, TimePoint, Timeline};
use re_types::{ComponentName, ComponentNameSet};

use crate::Caches;

// ---

// TODO: explain
#[derive(Clone, Copy)]
pub struct DataStoreRef<'a>(pub(crate) &'a DataStore);

impl<'a> From<&'a DataStore> for DataStoreRef<'a> {
    #[inline]
    fn from(store: &'a DataStore) -> Self {
        Self(store)
    }
}

impl Caches {
    /// Check whether a given entity has a specific [`ComponentName`] either on the specified
    /// timeline, or in its static data.
    #[inline]
    pub fn entity_has_component(
        &self,
        store: DataStoreRef<'_>,
        timeline: &Timeline,
        entity_path: &EntityPath,
        component_name: &ComponentName,
    ) -> bool {
        // TODO: justify (consistency and futureproofness in case we want to cache later).
        _ = self;

        store
            .0
            .entity_has_component(timeline, entity_path, component_name)
    }

    /// Retrieve all the [`ComponentName`]s that have been written to for a given [`EntityPath`] on
    /// the specified [`Timeline`].
    ///
    /// Static components are always included in the results.
    ///
    /// Returns `None` if the entity doesn't exist at all on this `timeline`.
    #[inline]
    pub fn all_components(
        &self,
        store: DataStoreRef<'_>,
        timeline: &Timeline,
        entity_path: &EntityPath,
    ) -> Option<ComponentNameSet> {
        // TODO: justify (consistency and futureproofness in case we want to cache later).
        _ = self;

        store.0.all_components(timeline, entity_path)
    }

    /// Lookup the arrow [`DataType`] of a [`re_types_core::Component`] in the internal
    /// `DataTypeRegistry`.
    #[inline]
    pub fn lookup_datatype<'a>(
        &self,
        store: DataStoreRef<'a>,
        component_nae: &ComponentName,
    ) -> Option<&'a DataType> {
        // TODO: justify (consistency and futureproofness in case we want to cache later).
        _ = self;

        store.0.lookup_datatype(component_nae)
    }

    #[inline]
    pub fn row_metadata<'a>(
        &self,
        store: DataStoreRef<'a>,
        row_id: &RowId,
    ) -> Option<&'a (TimePoint, EntityPathHash)> {
        // TODO: justify (consistency and futureproofness in case we want to cache later).
        _ = self;

        store.0.row_metadata(row_id)
    }

    /// Find the earliest time at which something was logged for a given entity on the specified
    /// timeline.
    ///
    /// Ignores static data.
    #[inline]
    pub fn entity_min_time(
        &self,
        store: DataStoreRef<'_>,
        timeline: &Timeline,
        entity_path: &EntityPath,
    ) -> Option<TimeInt> {
        // TODO: justify (consistency and futureproofness in case we want to cache later).
        _ = self;

        store.0.entity_min_time(timeline, entity_path)
    }
}

// TODO: these should be defined on the caches, even if they're not.
impl<'a> DataStoreRef<'a> {}
