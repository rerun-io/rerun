use std::sync::{Arc, OnceLock};

use nohash_hasher::IntMap;
use re_log_types::{DataCell, RowId, TimeInt};
use re_query2::{Promise, PromiseResolver, PromiseResult, QueryError};
use re_types_core::ComponentName;
use re_types_core::{Component, DeserializationError};

use crate::{ErasedFlatVecDeque, FlatVecDeque};

// ---

/// Results for a cached latest-at query.
#[derive(Debug)]
pub struct CachedLatestAtResults {
    pub max_index: (Option<TimeInt>, RowId),
    pub components: IntMap<ComponentName, Arc<CachedLatestAtComponentResults>>,
}

impl Default for CachedLatestAtResults {
    #[inline]
    fn default() -> Self {
        Self {
            max_index: (None, RowId::ZERO),
            components: Default::default(),
        }
    }
}

impl CachedLatestAtResults {
    #[inline]
    pub fn contains(&self, component_name: impl Into<ComponentName>) -> bool {
        self.components.contains_key(&component_name.into())
    }

    /// Returns the [`LatestAtComponentResults`] for the specified [`Component`].
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
            static DEFAULT: CachedLatestAtComponentResults = CachedLatestAtComponentResults::new();
            &DEFAULT
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
        let (data_time, row_id) = cached.index;
        let (max_data_time, max_row_id) = &mut self.max_index;

        // NOTE: Since this is a compound API that actually emits multiple queries, the data time of the
        // final result is the most recent data time among all of its components.
        if data_time > *max_data_time {
            *max_data_time = (*max_data_time).max(data_time);
            // TODO: max_row_id is a shit name
            *max_row_id = row_id;
        }

        self.components.insert(component_name, cached);
    }
}

// ---

/// Lazily cached results for a particular component when using a cached latest-at query.
pub struct CachedLatestAtComponentResults {
    pub(crate) index: (Option<TimeInt>, RowId),

    // Option so we can have a constant default value for `Self`.
    pub(crate) cell: Option<Promise>,

    // TODO: there shouldnt be any risk with this (implicit) lock
    pub(crate) cached: OnceLock<Box<dyn ErasedFlatVecDeque + Send + Sync>>,
}

impl CachedLatestAtComponentResults {
    #[inline]
    pub const fn new() -> Self {
        Self {
            index: (None, RowId::ZERO),
            cell: None,
            cached: OnceLock::new(),
        }
    }
}

impl std::fmt::Debug for CachedLatestAtComponentResults {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            index,
            cell,
            cached: _, // we can't, we don't know the type
        } = self;

        f.debug_struct("CachedComponentData")
            .field("index", &index)
            .field("cell", &cell)
            .finish()
    }
}

impl CachedLatestAtComponentResults {
    /// Returns the component data as a dense vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn to_dense<C: 'static + Component + Send + Sync>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> PromiseResult<crate::Result<&[C]>> {
        if let Some(cell) = self.cell.as_ref() {
            resolver
                .resolve(cell)
                .map_ok(|cell| self.downcast_dense::<C>(&cell))
        } else {
            // Manufactured empty result.
            PromiseResult::Ready(Ok(&[]))
        }
    }

    /// Iterates over the component data, assuming it is dense.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn iter_dense<C: 'static + Component + Send + Sync>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> PromiseResult<crate::Result<impl ExactSizeIterator<Item = &C>>> {
        self.to_dense(resolver)
            .map_ok(|data| data.map(|data| data.iter()))
    }

    /// Returns the component data as a sparse vector.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn to_sparse<C: 'static + Component + Send + Sync>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> PromiseResult<crate::Result<&[Option<C>]>> {
        if let Some(cell) = self.cell.as_ref() {
            resolver
                .resolve(cell)
                .map_ok(|cell| self.downcast_sparse::<C>(&cell))
        } else {
            // Manufactured empty result.
            PromiseResult::Ready(Ok(&[]))
        }
    }

    /// Iterates over the component data, assuming it is sparse.
    ///
    /// Returns an error if the component is missing or cannot be deserialized.
    #[inline]
    pub fn iter_sparse<C: 'static + Component + Send + Sync>(
        &self,
        resolver: &mut PromiseResolver,
    ) -> PromiseResult<crate::Result<impl ExactSizeIterator<Item = Option<&C>>>> {
        self.to_sparse(resolver)
            .map_ok(|data| data.map(|data| data.iter().map(Option::as_ref)))
    }
}

impl CachedLatestAtComponentResults {
    fn downcast_dense<C: 'static + Component + Send + Sync>(
        &self,
        cell: &DataCell,
    ) -> crate::Result<&[C]> {
        // TODO: this is non-blocking -- best-effort to avoid deserializing for no reason
        if let Some(cached) = self.cached.get() {
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
            return Ok(cached.iter().next().unwrap());
        }

        let data = cell
            .try_to_native::<C>()
            .map_err(|err| DeserializationError::DataCellError(err.to_string()))?;

        #[allow(clippy::borrowed_box)]
        let cached: &Box<dyn ErasedFlatVecDeque + Send + Sync> = self
            .cached
            .get_or_init(move || Box::new(FlatVecDeque::from(data)));

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

    fn downcast_sparse<C: 'static + Component + Send + Sync>(
        &self,
        cell: &DataCell,
    ) -> crate::Result<&[Option<C>]> {
        // TODO: this is non-blocking -- best-effort to avoid deserializing for no reason
        if let Some(cached) = self.cached.get() {
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
            return Ok(cached.iter().next().unwrap());
        }

        let data = cell
            .try_to_native_opt::<C>()
            .map_err(|err| DeserializationError::DataCellError(err.to_string()))?;

        #[allow(clippy::borrowed_box)]
        let cached: &Box<dyn ErasedFlatVecDeque + Send + Sync> = self
            .cached
            .get_or_init(move || Box::new(FlatVecDeque::from(data)));

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
}
