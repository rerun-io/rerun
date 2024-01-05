use re_log_types::{DataCell, DataRow, EntityPath, RowId, TimeInt, TimePoint, Timeline};

use re_types_core::{Component, ComponentName};

use crate::{DataStore, LatestAtQuery};

// --- Read ---

/// A [`Component`] versioned with a specific [`RowId`].
///
/// This is not enough to globally, uniquely identify an instance of a component.
/// For that you will need to combine the `InstancePath` that was used to query
/// the versioned component with the returned [`RowId`], therefore creating a
/// `VersionedInstancePath`.
#[derive(Debug, Clone)]
pub struct VersionedComponent<C: Component> {
    pub row_id: RowId,
    pub value: C,
}

impl<C: Component> From<(RowId, C)> for VersionedComponent<C> {
    #[inline]
    fn from((row_id, value): (RowId, C)) -> Self {
        Self { row_id, value }
    }
}

impl<C: Component> std::ops::Deref for VersionedComponent<C> {
    type Target = C;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl DataStore {
    /// Get the latest value for a given [`re_types_core::Component`], as well as the associated
    /// _data_ time and [`RowId`].
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will generate a log message of `level` otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log messages on failure.
    pub fn query_latest_component_with_log_level<C: Component>(
        &self,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
        level: re_log::Level,
    ) -> Option<(Option<TimeInt>, VersionedComponent<C>)> {
        re_tracing::profile_function!();

        let (data_time, row_id, cells) =
            self.latest_at(query, entity_path, C::name(), &[C::name()])?;
        let cell = cells.first()?.as_ref()?;

        cell.try_to_native_mono::<C>()
            .map_err(|err| {
                if let re_log_types::DataCellError::LoggableDeserialize(err) = err {
                    let bt = err.backtrace().map(|mut bt| {
                        bt.resolve();
                        bt
                    });

                    let err = Box::new(err) as Box<dyn std::error::Error>;
                    if let Some(bt) = bt {
                        re_log::log_once!(
                            level,
                            "Couldn't deserialize component at {entity_path}#{}: {}\n{:#?}",
                            C::name(),
                            re_error::format(&err),
                            bt,
                        );
                    } else {
                        re_log::log_once!(
                            level,
                            "Couldn't deserialize component at {entity_path}#{}: {}",
                            C::name(),
                            re_error::format(&err)
                        );
                    }
                    return err;
                }

                let err = Box::new(err) as Box<dyn std::error::Error>;
                re_log::log_once!(
                    level,
                    "Couldn't deserialize component at {entity_path}#{}: {}",
                    C::name(),
                    re_error::format(&err)
                );

                err
            })
            .ok()?
            .map(|c| (data_time, (row_id, c).into()))
    }

    /// Get the latest value for a given [`re_types_core::Component`], as well as the associated
    /// _data_ time and [`RowId`].
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will log a warning otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log errors on failure.
    #[inline]
    pub fn query_latest_component<C: Component>(
        &self,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
    ) -> Option<(Option<TimeInt>, VersionedComponent<C>)> {
        self.query_latest_component_with_log_level(entity_path, query, re_log::Level::Warn)
    }

    /// Get the latest value for a given [`re_types_core::Component`], as well as the associated
    /// _data_ time and [`RowId`].
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will return None and log a debug message otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely logs debug messages on failure.
    #[inline]
    pub fn query_latest_component_quiet<C: Component>(
        &self,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
    ) -> Option<(Option<TimeInt>, VersionedComponent<C>)> {
        self.query_latest_component_with_log_level(entity_path, query, re_log::Level::Debug)
    }

    /// Call [`Self::query_latest_component`] at the given path, walking up the hierarchy until an instance is found.
    pub fn query_latest_component_at_closest_ancestor<C: Component>(
        &self,
        entity_path: &EntityPath,
        query: &LatestAtQuery,
    ) -> Option<(EntityPath, Option<TimeInt>, VersionedComponent<C>)> {
        re_tracing::profile_function!();

        let mut cur_path = Some(entity_path.clone());
        while let Some(path) = cur_path {
            if let Some((data_time, c)) = self.query_latest_component::<C>(&path, query) {
                return Some((path, data_time, c));
            }
            cur_path = path.parent();
        }
        None
    }

    /// Get the latest value for a given [`re_types_core::Component`] and the associated [`RowId`],
    /// assuming it is timeless.
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will log a warning otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log errors on failure.
    pub fn query_timeless_component<C: Component>(
        &self,
        entity_path: &EntityPath,
    ) -> Option<VersionedComponent<C>> {
        re_tracing::profile_function!();

        let query = LatestAtQuery::latest(Timeline::default());
        self.query_latest_component(entity_path, &query)
            .map(|(data_time, vc)| {
                debug_assert!(data_time.is_none());
                vc
            })
    }

    /// Get the latest value for a given [`re_types_core::Component`] and the associated [`RowId`],
    /// assuming it is timeless.
    ///
    /// This assumes that the row we get from the store only contains a single instance for this
    /// component; it will return None and log a debug message otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// This is a best-effort helper, it will merely log debug on failure.
    pub fn query_timeless_component_quiet<C: Component>(
        &self,
        entity_path: &EntityPath,
    ) -> Option<VersionedComponent<C>> {
        re_tracing::profile_function!();

        let query = LatestAtQuery::latest(Timeline::default());
        self.query_latest_component_quiet(entity_path, &query)
            .map(|(data_time, vc)| {
                debug_assert!(data_time.is_none());
                vc
            })
    }
}

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
