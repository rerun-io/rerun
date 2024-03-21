use re_log_types::{DataCell, DataRow, EntityPath, RowId, TimePoint};
use re_types_core::{Component, ComponentName};

use crate::DataStore;

// --- Write ---

impl DataStore {
    /// Stores a single value for a given [`re_types_core::Component`].
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

        let mut row = match DataRow::from_cells1(
            RowId::new(),
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

    /// Stores a single empty value for a given [`re_types_core::ComponentName`].
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

            let mut row = match DataRow::from_cells1(
                RowId::new(),
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
