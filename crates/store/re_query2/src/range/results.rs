use std::{
    cell::RefCell,
    collections::VecDeque,
    ops::Range,
    sync::{Arc, OnceLock},
};

use arrow2::array::Array as ArrowArray;
use nohash_hasher::IntMap;
use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

use re_chunk::{Chunk, ChunkShared, RowId};
use re_chunk_store::RangeQuery;
use re_log_types::{ResolvedTimeRange, TimeInt};
use re_types_core::{Component, ComponentName, DeserializationError, SizeBytes};

use crate::{ErasedFlatVecDeque, FlatVecDeque, PromiseResult};

// ---

/// Results for a range query.
///
/// The data is both deserialized and resolved/converted.
///
/// Use [`RangeResults::get`], [`RangeResults::get_required`] and
/// [`RangeResults::get_or_empty`] in order to access the results for each individual component.
#[derive(Debug)]
pub struct RangeResults {
    pub query: RangeQuery,

    // TODO: Arc, probably
    pub components: IntMap<ComponentName, Vec<Chunk>>,
}

impl RangeResults {
    #[inline]
    pub fn new(query: RangeQuery) -> Self {
        Self {
            query,
            components: Default::default(),
        }
    }

    #[inline]
    pub fn contains(&self, component_name: &ComponentName) -> bool {
        self.components.contains_key(component_name)
    }

    /// Returns the [`RangeComponentResults`] for the specified [`Component`].
    #[inline]
    pub fn get(&self, component_name: &ComponentName) -> Option<&[Chunk]> {
        self.components
            .get(component_name)
            .map(|chunks| chunks.as_slice())
    }

    /// Returns the [`RangeComponentResults`] for the specified [`Component`].
    ///
    /// Returns an error if the component is not present.
    #[inline]
    pub fn get_required(&self, component_name: &ComponentName) -> crate::Result<&[Chunk]> {
        if let Some(chunks) = self.components.get(&component_name) {
            Ok(chunks)
        } else {
            Err(DeserializationError::MissingComponent {
                component: *component_name,
                backtrace: ::backtrace::Backtrace::new_unresolved(),
            }
            .into())
        }
    }

    /// Returns the [`RangeComponentResults`] for the specified [`Component`].
    ///
    /// Returns empty results if the component is not present.
    #[inline]
    pub fn get_or_empty(&self, component_name: &ComponentName) -> &[Chunk] {
        if let Some(chunks) = self.components.get(&component_name) {
            chunks
        } else {
            &[]
        }
    }
}

impl RangeResults {
    #[doc(hidden)]
    #[inline]
    pub fn add(&mut self, component_name: ComponentName, chunks: Vec<Chunk>) {
        self.components.insert(component_name, chunks);
    }
}
