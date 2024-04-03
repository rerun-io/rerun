use nohash_hasher::IntMap;
use re_log_types::{DataCell, RowId, TimeInt};
use re_types_core::ComponentName;
use re_types_core::{Component, DeserializationError, DeserializationResult};

use crate::{Promise, PromiseResolver, PromiseResult};

// ---

/// Raw results for a range query.
///
/// The data is neither deserialized, nor resolved/converted.
/// It it the raw [`DataCell`]s, straight from our datastore.
///
/// Use [`RangeResults::get`], [`RangeResults::get_required`] and [`RangeResults::get_optional`]
/// in order to access the raw results for each individual component.
#[derive(Default, Debug, Clone)]
pub struct RangeResults {
    /// Raw results for each individual component.
    pub components: IntMap<ComponentName, RangeComponentResults>,
}

impl RangeResults {
    #[inline]
    pub fn contains(&self, component_name: impl Into<ComponentName>) -> bool {
        self.components.contains_key(&component_name.into())
    }

    /// Returns the [`RangeComponentResults`] for the specified `component_name`.
    #[inline]
    pub fn get(&self, component_name: impl Into<ComponentName>) -> Option<&RangeComponentResults> {
        self.components.get(&component_name.into())
    }

    /// Returns the [`RangeComponentResults`] for the specified `component_name`.
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required(
        &self,
        component_name: impl Into<ComponentName>,
    ) -> crate::Result<&RangeComponentResults> {
        let component_name = component_name.into();
        if let Some(component) = self.components.get(&component_name) {
            Ok(component)
        } else {
            Err(DeserializationError::MissingComponent {
                component: component_name,
                backtrace: ::backtrace::Backtrace::new_unresolved(),
            }
            .into())
        }
    }

    /// Returns the [`RangeComponentResults`] for the specified `component_name`.
    ///
    /// Returns empty results if the component is not present.
    #[inline]
    pub fn get_optional(&self, component_name: impl Into<ComponentName>) -> &RangeComponentResults {
        let component_name = component_name.into();
        if let Some(component) = self.components.get(&component_name) {
            component
        } else {
            static DEFAULT: RangeComponentResults = RangeComponentResults::empty();
            &DEFAULT
        }
    }
}

impl RangeResults {
    #[doc(hidden)]
    #[inline]
    pub fn add(
        &mut self,
        component_name: ComponentName,
        data: impl Iterator<Item = ((TimeInt, RowId), DataCell)>,
    ) {
        let (indices, cells): (Vec<_>, Vec<_>) = data
            .map(|(index, cell)| (index, Promise::new(cell)))
            .unzip();

        let results = RangeComponentResults {
            indices,
            promises: cells,
        };
        results.sanity_check();

        self.components.insert(component_name, results);
    }
}

// ---

/// Uncached results for a particular component when using a range query.
#[derive(Debug, Clone)]
pub struct RangeComponentResults {
    pub indices: Vec<(TimeInt, RowId)>,
    pub promises: Vec<Promise>,
}

impl Default for RangeComponentResults {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

impl RangeComponentResults {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            indices: Vec::new(),
            promises: Vec::new(),
        }
    }

    /// No-op in release.
    #[inline]
    pub fn sanity_check(&self) {
        let Self {
            indices,
            promises: cells,
        } = self;
        if cfg!(debug_assertions) {
            assert_eq!(indices.len(), cells.len());
        }
    }
}

impl RangeComponentResults {
    #[inline]
    pub fn indices(&self) -> &[(TimeInt, RowId)] {
        &self.indices
    }

    #[inline]
    pub fn iter_indices(&self) -> impl ExactSizeIterator<Item = (TimeInt, RowId)> + '_ {
        self.indices.iter().copied()
    }

    /// Returns the component data as a vector of dense vectors.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn to_dense<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> Vec<PromiseResult<DeserializationResult<Vec<C>>>> {
        self.promises
            .iter()
            .map(|cell| {
                resolver.resolve(cell).map(|cell| {
                    cell.try_to_native()
                        .map_err(|err| DeserializationError::DataCellError(err.to_string()))
                })
            })
            .collect()
    }

    /// Returns the component data as an iterator of dense vectors.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn iter_dense<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> impl ExactSizeIterator<Item = PromiseResult<DeserializationResult<Vec<C>>>> {
        self.to_dense(resolver).into_iter()
    }

    /// Returns the component data as a vector of sparse vectors.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn to_sparse<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> Vec<PromiseResult<DeserializationResult<Vec<Option<C>>>>> {
        self.promises
            .iter()
            .map(|cell| {
                resolver.resolve(cell).map(|cell| {
                    cell.try_to_native_opt()
                        .map_err(|err| DeserializationError::DataCellError(err.to_string()))
                })
            })
            .collect()
    }

    /// Returns the component data as an iterator of sparse vectors.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn iter_sparse<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> impl ExactSizeIterator<Item = PromiseResult<DeserializationResult<Vec<Option<C>>>>> {
        self.to_sparse(resolver).into_iter()
    }
}
