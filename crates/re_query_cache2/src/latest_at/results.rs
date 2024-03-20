use std::sync::{
    atomic::{AtomicU64, Ordering::Relaxed},
    Arc, OnceLock,
};

use nohash_hasher::IntMap;

use re_log_types::{DataCell, RowId, TimeInt};
use re_types_core::{Component, ComponentName, DeserializationError, SizeBytes};

use crate::{
    ErasedFlatVecDeque, FlatVecDeque, Promise, PromiseResolver, PromiseResult, QueryError,
};

// ---

/// Cached results for a latest-at query.
///
/// The data is both deserialized and resolved/converted.
///
/// Use [`CachedLatestAtResults::get`], [`CachedLatestAtResults::get_required`] and
/// [`CachedLatestAtResults::get_optional`] in order to access the results for each individual component.
#[derive(Debug)]
pub struct CachedLatestAtResults {
    /// The compound index of this query result.
    ///
    /// A latest-at query is a compound operation that gathers data from many different rows.
    /// The index of that compound result corresponds to the index of most the recent row in all the
    /// sub-results, as defined by time and row-id order.
    pub compound_index: (TimeInt, RowId),

    /// Results for each individual component.
    pub components: IntMap<ComponentName, Arc<CachedLatestAtComponentResults>>,
}

impl Default for CachedLatestAtResults {
    #[inline]
    fn default() -> Self {
        Self {
            compound_index: (TimeInt::STATIC, RowId::ZERO),
            components: Default::default(),
        }
    }
}

impl CachedLatestAtResults {
    #[inline]
    pub fn contains(&self, component_name: impl Into<ComponentName>) -> bool {
        self.components.contains_key(&component_name.into())
    }

    /// Returns the [`CachedLatestAtComponentResults`] for the specified [`Component`].
    #[inline]
    pub fn get<C: Component>(&self) -> Option<&CachedLatestAtComponentResults> {
        self.components.get(&C::name()).map(|arc| &**arc)
    }

    /// Returns the [`CachedLatestAtComponentResults`] for the specified [`Component`].
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required<C: Component>(&self) -> crate::Result<&CachedLatestAtComponentResults> {
        if let Some(component) = self.components.get(&C::name()) {
            Ok(component)
        } else {
            Err(DeserializationError::MissingComponent {
                component: C::name(),
                backtrace: ::backtrace::Backtrace::new_unresolved(),
            }
            .into())
        }
    }

    /// Returns the [`CachedLatestAtComponentResults`] for the specified [`Component`].
    ///
    /// Returns empty results if the component is not present.
    #[inline]
    pub fn get_optional<C: Component>(&self) -> &CachedLatestAtComponentResults {
        if let Some(component) = self.components.get(&C::name()) {
            component
        } else {
            static EMPTY: CachedLatestAtComponentResults = CachedLatestAtComponentResults::empty();
            &EMPTY
        }
    }
}

impl CachedLatestAtResults {
    #[doc(hidden)]
    #[inline]
    pub fn add(
        &mut self,
        component_name: ComponentName,
        cached: Arc<CachedLatestAtComponentResults>,
    ) {
        // NOTE: Since this is a compound API that actually emits multiple queries, the index of the
        // final result is the most recent index among all of its components, as defined by time
        // and row-id order.
        //
        // TODO(#5303): We have to ignore the cluster key in this piece of logic for backwards compatibility
        // reasons with the legacy instance-key model. This will go away next.
        use re_types_core::Loggable as _;
        if component_name != re_types_core::components::InstanceKey::name()
            && cached.index > self.compound_index
        {
            self.compound_index = cached.index;
        }

        self.components.insert(component_name, cached);
    }
}

// ---

/// Lazily cached results for a particular component when using a cached latest-at query.
pub struct CachedLatestAtComponentResults {
    pub(crate) index: (TimeInt, RowId),

    // Option so we can have a constant default value for `Self`.
    pub(crate) promise: Option<Promise>,

    /// The resolved, converted, deserialized dense data.
    pub(crate) cached_dense: OnceLock<Box<dyn ErasedFlatVecDeque + Send + Sync>>,

    /// The resolved, converted, deserialized sparse data.
    pub(crate) cached_sparse: OnceLock<Box<dyn ErasedFlatVecDeque + Send + Sync>>,

    pub(crate) cached_heap_size_bytes: AtomicU64,
}

impl CachedLatestAtComponentResults {
    #[inline]
    pub const fn empty() -> Self {
        Self {
            index: (TimeInt::STATIC, RowId::ZERO),
            promise: None,
            cached_dense: OnceLock::new(),
            cached_sparse: OnceLock::new(),
            cached_heap_size_bytes: AtomicU64::new(0),
        }
    }
}

impl SizeBytes for CachedLatestAtComponentResults {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        self.cached_heap_size_bytes.load(Relaxed)
    }
}

impl std::fmt::Debug for CachedLatestAtComponentResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            index,
            promise: _,
            cached_dense: _,  // we can't, we don't know the type
            cached_sparse: _, // we can't, we don't know the type
            cached_heap_size_bytes,
        } = self;

        f.write_fmt(format_args!(
            "[{:?}#{}] {}",
            index.0,
            index.1,
            re_format::format_bytes(cached_heap_size_bytes.load(Relaxed) as _)
        ))
    }
}

impl CachedLatestAtComponentResults {
    #[inline]
    pub fn index(&self) -> &(TimeInt, RowId) {
        &self.index
    }

    /// Returns the component data as a dense vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn to_dense<C: 'static + Component + Send + Sync>(
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
    pub fn iter_dense<C: 'static + Component + Send + Sync>(
        &self,
        resolver: &PromiseResolver,
    ) -> PromiseResult<crate::Result<impl ExactSizeIterator<Item = &C>>> {
        self.to_dense(resolver)
            .map(|data| data.map(|data| data.iter()))
    }

    /// Returns the component data as a sparse vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn to_sparse<C: 'static + Component + Send + Sync>(
        &self,
        resolver: &PromiseResolver,
    ) -> PromiseResult<crate::Result<&[Option<C>]>> {
        if let Some(cell) = self.promise.as_ref() {
            resolver
                .resolve(cell)
                .map(|cell| self.downcast_sparse::<C>(&cell))
        } else {
            // Manufactured empty result.
            PromiseResult::Ready(Ok(&[]))
        }
    }

    /// Iterates over the component data, assuming it is sparse.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    ///
    /// Use [`PromiseResult::flatten`] to merge the results of resolving the promise and of
    /// deserializing the data into a single one, if you don't need the extra flexibility.
    #[inline]
    pub fn iter_sparse<C: 'static + Component + Send + Sync>(
        &self,
        resolver: &PromiseResolver,
    ) -> PromiseResult<crate::Result<impl ExactSizeIterator<Item = Option<&C>>>> {
        self.to_sparse(resolver)
            .map(|data| data.map(|data| data.iter().map(Option::as_ref)))
    }
}

impl CachedLatestAtComponentResults {
    fn downcast_dense<C: 'static + Component + Send + Sync>(
        &self,
        cell: &DataCell,
    ) -> crate::Result<&[C]> {
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
        let cached: &Box<dyn ErasedFlatVecDeque + Send + Sync> =
            self.cached_dense.get_or_init(move || {
                self.cached_heap_size_bytes
                    .fetch_add(data.total_size_bytes(), Relaxed);
                Box::new(FlatVecDeque::from(data))
            });

        downcast(&**cached)
    }

    fn downcast_sparse<C: 'static + Component + Send + Sync>(
        &self,
        cell: &DataCell,
    ) -> crate::Result<&[Option<C>]> {
        // `OnceLock::get` is non-blocking -- this is a best-effort fast path in case the
        // data has already been computed.
        //
        // See next comment as to why we need this.
        if let Some(cached) = self.cached_sparse.get() {
            return downcast_opt(&**cached);
        }

        // We have to do this outside of the callback in order to propagate errors.
        // Hence the early exit check above.
        let data = cell
            .try_to_native_opt::<C>()
            .map_err(|err| DeserializationError::DataCellError(err.to_string()))?;

        #[allow(clippy::borrowed_box)]
        let cached: &Box<dyn ErasedFlatVecDeque + Send + Sync> =
            self.cached_sparse.get_or_init(move || {
                self.cached_heap_size_bytes
                    .fetch_add(data.total_size_bytes(), Relaxed);
                Box::new(FlatVecDeque::from(data))
            });

        downcast_opt(&**cached)
    }
}

fn downcast<C: 'static + Component + Send + Sync>(
    cached: &(dyn ErasedFlatVecDeque + Send + Sync),
) -> crate::Result<&[C]> {
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
    // unwrap checked just above ^^^
    Ok(cached.iter().next().unwrap())
}

fn downcast_opt<C: 'static + Component + Send + Sync>(
    cached: &(dyn ErasedFlatVecDeque + Send + Sync),
) -> crate::Result<&[Option<C>]> {
    let cached = cached
        .as_any()
        .downcast_ref::<FlatVecDeque<Option<C>>>()
        .ok_or_else(|| QueryError::TypeMismatch {
            actual: "<unknown>".into(),
            requested: C::name(),
        })?;

    if cached.num_entries() != 1 {
        return Err(anyhow::anyhow!("latest_at deque must be single entry").into());
    }
    // unwrap checked just above ^^^
    Ok(cached.iter().next().unwrap())
}
