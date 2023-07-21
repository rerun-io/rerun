use re_log_types::{DataCell, DataRow, EntityPath, RowId, TimePoint, Timeline};

use re_types::{Component, ComponentName};

use crate::{DataStore, LatestAtQuery};

// --- Read ---

impl DataStore {
    /// Get the latest value for a given [`re_types::Component`].
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will log a warning otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log errors on failure.
    pub fn query_latest_component<C: Component>(
        &self,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
    ) -> Option<C> {
        re_tracing::profile_function!();

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

    /// Call `query_latest_component` at the given path, walking up the hierarchy until an instance is found.
    pub fn query_latest_component_at_closest_ancestor<C: Component>(
        &self,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
    ) -> Option<(EntityPath, C)> {
        re_tracing::profile_function!();

        let mut cur_path = Some(entity_path.clone());
        while let Some(path) = cur_path {
            if let Some(component) = self.query_latest_component::<C>(&path, query) {
                return Some((path, component));
            }
            cur_path = path.parent();
        }
        None
    }

    /// Get the latest value for a given [`re_types::Component`], assuming it is timeless.
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will log a warning otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log errors on failure.
    pub fn query_timeless_component<C: Component>(&self, entity_path: &EntityPath) -> Option<C> {
        re_tracing::profile_function!();

        let query = LatestAtQuery::latest(Timeline::default());
        self.query_latest_component(entity_path, &query)
    }
}

// --- Write ---

impl DataStore {
    /// Stores a single value for a given [`re_types::Component`].
    ///
    /// This is a best-effort helper, it will merely log errors on failure.
    pub fn insert_component<'a, C>(
        &mut self,
        entity_path: &EntityPath,
        timepoint: &TimePoint,
        component: C,
    ) where
        C: Component + Clone + 'a,
        std::borrow::Cow<'a, C>: std::convert::From<C>,
    {
        re_tracing::profile_function!();

        let mut row = match DataRow::try_from_cells1(
            RowId::random(),
            entity_path.clone(),
            timepoint.clone(),
            1,
            [component],
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
        re_tracing::profile_function!();

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
