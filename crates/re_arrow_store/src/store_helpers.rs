use re_log_types::{
    ComponentName, DataCell, DataRow, DeserializableComponent, EntityPath, RowId,
    SerializableComponent, TimeInt, TimePoint, Timeline,
};

use crate::{DataStore, LatestAtQuery};

// --- Read ---

impl DataStore {
    /// Get the latest value for a given [`re_log_types::Component`].
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will log a warning otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log errors on failure.
    pub fn query_latest_component<C: DeserializableComponent>(
        &self,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
    ) -> Option<C>
    where
        for<'b> &'b C::ArrayType: IntoIterator,
    {
        crate::profile_function!();

        let (_, cells) = self.latest_at(query, entity_path, C::name(), &[C::name()])?;
        let cell = cells.get(0)?.as_ref()?;

        let mut iter = cell
            .try_to_native::<C>()
            .map_err(|err| {
                re_log::error_once!(
                    "Couldn't deserialize component at {entity_path}.{}: {err}",
                    C::name()
                );
            })
            .ok()?;

        let component = iter.next();

        if iter.next().is_some() {
            re_log::warn_once!("Unexpected batch for {} at: {}", C::name(), entity_path);
        }

        component
    }

    /// Get the latest value for a given [`re_log_types::Component`], assuming it is timeless.
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will log a warning otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log errors on failure.
    pub fn query_timeless_component<C: DeserializableComponent>(
        &self,
        entity_path: &EntityPath,
    ) -> Option<C>
    where
        for<'b> &'b C::ArrayType: IntoIterator,
    {
        crate::profile_function!();

        let query = LatestAtQuery::new(Timeline::default(), TimeInt::MAX);
        self.query_latest_component(entity_path, &query)
    }
}

// --- Write ---

impl DataStore {
    /// Stores a single value for a given [`re_log_types::Component`].
    ///
    /// This is a best-effort helper, it will merely log errors on failure.
    pub fn insert_component<C: SerializableComponent>(
        &mut self,
        entity_path: &EntityPath,
        timepoint: &TimePoint,
        component: C,
    ) {
        crate::profile_function!();

        let mut row = match DataRow::try_from_cells1(
            RowId::random(),
            entity_path.clone(),
            timepoint.clone(),
            1,
            [component].as_slice(),
        ) {
            Ok(row) => row,
            Err(err) => {
                re_log::error_once!(
                    "Couldn't serialize component at {entity_path}.{}: {err}",
                    C::name()
                );
                return;
            }
        };
        row.compute_all_size_bytes();

        if let Err(err) = self.insert_row(&row) {
            re_log::error_once!(
                "Couldn't insert component at {entity_path}.{}: {err}",
                C::name()
            );
        }
    }

    /// Stores a single empty value for a given [`re_log_types::ComponentName`].
    ///
    /// This is a best-effort helper, it will merely log errors on failure.
    pub fn insert_empty_component(
        &mut self,
        entity_path: &EntityPath,
        timepoint: &TimePoint,
        component: ComponentName,
    ) {
        crate::profile_function!();

        if let Some(datatype) = self.lookup_datatype(&component) {
            let cell = DataCell::from_arrow_empty(component, datatype.clone());

            let mut row = match DataRow::try_from_cells1(
                RowId::random(),
                entity_path.clone(),
                timepoint.clone(),
                cell.num_instances(),
                cell,
            ) {
                Ok(row) => row,
                Err(err) => {
                    re_log::error_once!(
                        "Couldn't serialize component at {entity_path}.{}: {err}",
                        component
                    );
                    return;
                }
            };
            row.compute_all_size_bytes();

            if let Err(err) = self.insert_row(&row) {
                re_log::error_once!(
                    "Couldn't insert component at {entity_path}.{}: {err}",
                    component
                );
            }
        } else {
            re_log::error_once!(
                "Couldn't find appropriate datatype at {entity_path}.{}",
                component
            );
        }
    }
}
