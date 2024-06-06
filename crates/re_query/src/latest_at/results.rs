use std::sync::{Arc, OnceLock};

use nohash_hasher::IntMap;

use re_log_types::{DataCell, RowId, TimeInt};
use re_types_core::{Component, ComponentName, DeserializationError, SizeBytes};

use crate::{
    ErasedFlatVecDeque, FlatVecDeque, Promise, PromiseResolver, PromiseResult, QueryError,
};

// ---

/// Results for a latest-at query.
///
/// The data is both deserialized and resolved/converted.
///
/// Use [`LatestAtResults::get`], [`LatestAtResults::get_required`] and
/// [`LatestAtResults::get_or_empty`] in order to access the results for each individual component.
#[derive(Debug)]
pub struct LatestAtResults {
    /// The compound index of this query result.
    ///
    /// A latest-at query is a compound operation that gathers data from many different rows.
    /// The index of that compound result corresponds to the index of most the recent row in all the
    /// sub-results, as defined by time and row-id order.
    pub compound_index: (TimeInt, RowId),

    /// Results for each individual component.
    pub components: IntMap<ComponentName, Arc<LatestAtComponentResults>>,
}

impl Default for LatestAtResults {
    #[inline]
    fn default() -> Self {
        Self {
            compound_index: (TimeInt::STATIC, RowId::ZERO),
            components: Default::default(),
        }
    }
}

impl LatestAtResults {
    #[inline]
    pub fn contains(&self, component_name: impl Into<ComponentName>) -> bool {
        self.components.contains_key(&component_name.into())
    }

    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
    #[inline]
    pub fn get(
        &self,
        component_name: impl Into<ComponentName>,
    ) -> Option<&LatestAtComponentResults> {
        self.components
            .get(&component_name.into())
            .map(|arc| &**arc)
    }

    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required(
        &self,
        component_name: impl Into<ComponentName>,
    ) -> crate::Result<&LatestAtComponentResults> {
        let component_name = component_name.into();
        if let Some(component) = self.components.get(&component_name) {
            Ok(component)
        } else {
            Err(QueryError::PrimaryNotFound(component_name))
        }
    }

    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
    ///
    /// Returns empty results if the component is not present.
    #[inline]
    pub fn get_or_empty(
        &self,
        component_name: impl Into<ComponentName>,
    ) -> &LatestAtComponentResults {
        let component_name = component_name.into();
        if let Some(component) = self.components.get(&component_name) {
            component
        } else {
            static EMPTY: LatestAtComponentResults = LatestAtComponentResults::empty();
            &EMPTY
        }
    }

    /// Utility for retrieving a single instance of a component.
    pub fn get_instance<T: re_types_core::Component>(
        &self,
        resolver: &PromiseResolver,
        index: usize,
    ) -> Option<T> {
        self.get(T::name())
            .and_then(|r| r.try_instance::<T>(resolver, index))
    }

    /// Utility for retrieving a specific component.
    pub fn get_slice<T: re_types_core::Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> Option<&[T]> {
        self.get(T::name()).and_then(|r| r.dense::<T>(resolver))
    }

    pub fn get_vec<T: re_types_core::Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> Option<Vec<T>> {
        self.get_slice(resolver).map(|v| v.to_vec())
    }
}

impl LatestAtResults {
    #[doc(hidden)]
    #[inline]
    pub fn add(&mut self, component_name: ComponentName, cached: Arc<LatestAtComponentResults>) {
        // NOTE: Since this is a compound API that actually emits multiple queries, the index of the
        // final result is the most recent index among all of its components, as defined by time
        // and row-id order.

        if cached.index > self.compound_index {
            self.compound_index = cached.index;
        }

        self.components.insert(component_name, cached);
    }
}

// ---

/// Lazily cached results for a particular component when using a cached latest-at query.
pub struct LatestAtComponentResults {
    pub(crate) index: (TimeInt, RowId),

    // Option so we can have a constant default value for `Self`.
    pub(crate) promise: Option<Promise>,

    /// The resolved, converted, deserialized dense data.
    pub(crate) cached_dense: OnceLock<Arc<dyn ErasedFlatVecDeque + Send + Sync>>,
}

impl LatestAtComponentResults {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            index: (TimeInt::STATIC, RowId::ZERO),
            promise: None,
            cached_dense: OnceLock::new(),
        }
    }

    /// Returns the [`ComponentName`] of the resolved data, if available.
    #[inline]
    pub fn component_name(&self, resolver: &PromiseResolver) -> Option<ComponentName> {
        match self.resolved(resolver) {
            PromiseResult::Ready(cell) => Some(cell.component_name()),
            _ => None,
        }
    }

    /// Returns whether the resolved data is static.
    #[inline]
    pub fn is_static(&self) -> bool {
        self.index.0 == TimeInt::STATIC
    }

    /// How many _indices_ across this entire cache?
    #[inline]
    pub fn num_indices(&self) -> u64 {
        _ = self;
        1
    }

    /// How many _instances_ across this entire cache?
    #[inline]
    pub fn num_instances(&self) -> u64 {
        self.cached_dense
            .get()
            .map_or(0u64, |cached| cached.dyn_num_values() as _)
    }
}

impl SizeBytes for LatestAtComponentResults {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        let Self {
            index,
            promise,
            cached_dense,
        } = self;

        index.total_size_bytes()
            + promise.total_size_bytes()
            + cached_dense
                .get()
                .map_or(0, |data| data.dyn_total_size_bytes())
    }
}

impl std::fmt::Debug for LatestAtComponentResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            index,
            promise: _,
            cached_dense: _, // we can't, we don't know the type
        } = self;

        f.write_fmt(format_args!(
            "[{:?}#{}] {}",
            index.0,
            index.1,
            re_format::format_bytes(self.total_size_bytes() as _)
        ))
    }
}

impl LatestAtComponentResults {
    #[inline]
    pub fn index(&self) -> &(TimeInt, RowId) {
        &self.index
    }

    /// Returns the raw resolved data, if it's ready.
    #[inline]
    pub fn resolved(&self, resolver: &PromiseResolver) -> PromiseResult<DataCell> {
        if let Some(cell) = self.promise.as_ref() {
            resolver.resolve(cell)
        } else {
            PromiseResult::Pending
        }
    }

    /// Returns the component data as a dense vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn to_dense<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> PromiseResult<crate::Result<&[C]>> {
        if let Some(cell) = self.promise.as_ref() {
            resolver
                .resolve(cell)
                .map(|cell| self.downcast_dense::<C>(&cell))
        } else {
            // Manufactured empty result.
            PromiseResult::Ready(Ok(&[]))
        }
    }

    /// Iterates over the component data, assuming it is dense.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn iter_dense<C: Component>(
        &self,
        resolver: &PromiseResolver,
    ) -> PromiseResult<crate::Result<impl ExactSizeIterator<Item = &C>>> {
        self.to_dense(resolver)
            .map(|data| data.map(|data| data.iter()))
    }
}

impl LatestAtComponentResults {
    fn downcast_dense<C: Component>(&self, cell: &DataCell) -> crate::Result<&[C]> {
        // `OnceLock::get` is non-blocking -- this is a best-effort fast path in case the
        // data has already been computed.
        //
        // See next comment as to why we need this.
        if let Some(cached) = self.cached_dense.get() {
            return downcast(&**cached);
        }

        // We have to do this outside of the callback in order to propagate errors.
        // Hence the early exit check above.
        let data = cell
            .try_to_native::<C>()
            .map_err(|err| DeserializationError::DataCellError(err.to_string()))?;

        #[allow(clippy::borrowed_box)]
        let cached: &Arc<dyn ErasedFlatVecDeque + Send + Sync> = self
            .cached_dense
            .get_or_init(move || Arc::new(FlatVecDeque::from(data)));

        downcast(&**cached)
    }
}

fn downcast<C: Component>(cached: &(dyn ErasedFlatVecDeque + Send + Sync)) -> crate::Result<&[C]> {
    let cached = cached
        .as_any()
        .downcast_ref::<FlatVecDeque<C>>()
        .ok_or_else(|| QueryError::TypeMismatch {
            actual: "<unknown>".into(),
            requested: C::name(),
        })?;

    if cached.num_entries() != 1 {
        return Err(anyhow::anyhow!("latest_at deque must be single entry").into());
    }
    Ok(cached
        .iter()
        .next()
        .expect("checked existence of cached value already"))
}
