use std::sync::Arc;

use arrow2::array::Array as ArrowArray;

use re_log_types::{TimeInt, Timeline};
use re_types_core::{Component, ComponentName, SizeBytes};

use crate::{Chunk, ChunkResult, RowId};

// --- Helpers ---

impl Chunk {
    // --- Batch ---

    /// Returns the raw data for the specified component.
    ///
    /// Returns an error if the row index is out of bounds.
    #[inline]
    pub fn component_batch_raw(
        &self,
        component_name: &ComponentName,
        row_index: usize,
    ) -> Option<ChunkResult<Box<dyn ArrowArray>>> {
        self.components.get(component_name).map(|list_array| {
            if list_array.len() > row_index {
                Ok(list_array.value(row_index))
            } else {
                Err(crate::ChunkError::IndexOutOfBounds {
                    kind: "row".to_owned(),
                    len: list_array.len(),
                    index: row_index,
                })
            }
        })
    }

    /// Returns the deserialized data for the specified component.
    ///
    /// Returns an error if the data cannot be deserialized, or if the row index is out of bounds.
    #[inline]
    pub fn component_batch<C: Component>(&self, row_index: usize) -> Option<ChunkResult<Vec<C>>> {
        let res = self.component_batch_raw(&C::name(), row_index)?;

        let array = match res {
            Ok(array) => array,
            Err(err) => return Some(Err(err)),
        };

        let data = C::from_arrow(&*array);
        Some(data.map_err(Into::into))
    }

    // --- Instance ---

    /// Returns the raw data for the specified component at the given instance index.
    ///
    /// Returns an error if either the row index or instance index are out of bounds.
    #[inline]
    pub fn component_instance_raw(
        &self,
        component_name: &ComponentName,
        row_index: usize,
        instance_index: usize,
    ) -> Option<ChunkResult<Box<dyn ArrowArray>>> {
        let res = self.component_batch_raw(component_name, row_index)?;

        let array = match res {
            Ok(array) => array,
            Err(err) => return Some(Err(err)),
        };

        if array.len() > instance_index {
            Some(Ok(array.sliced(instance_index, 1)))
        } else {
            Some(Err(crate::ChunkError::IndexOutOfBounds {
                kind: "instance".to_owned(),
                len: array.len(),
                index: instance_index,
            }))
        }
    }

    /// Returns the component data of the specified instance.
    ///
    /// Returns an error if the data cannot be deserialized, or if either the row index or instance index
    /// are out of bounds.
    #[inline]
    pub fn component_instance<C: Component>(
        &self,
        row_index: usize,
        instance_index: usize,
    ) -> Option<ChunkResult<C>> {
        let res = self.component_instance_raw(&C::name(), row_index, instance_index)?;

        let array = match res {
            Ok(array) => array,
            Err(err) => return Some(Err(err)),
        };

        match C::from_arrow(&*array) {
            Ok(data) => data.into_iter().next().map(Ok), // NOTE: It's already sliced!
            Err(err) => Some(Err(err.into())),
        }
    }

    // --- Mono ---

    /// Returns the raw data for the specified component, assuming a mono-batch.
    ///
    /// Returns an error if either the row index is out of bounds, or the underlying batch is not
    /// of unit length.
    #[inline]
    pub fn component_mono_raw(
        &self,
        component_name: &ComponentName,
        row_index: usize,
    ) -> Option<ChunkResult<Box<dyn ArrowArray>>> {
        let res = self.component_batch_raw(component_name, row_index)?;

        let array = match res {
            Ok(array) => array,
            Err(err) => return Some(Err(err)),
        };

        if array.len() == 1 {
            Some(Ok(array.sliced(0, 1)))
        } else {
            Some(Err(crate::ChunkError::IndexOutOfBounds {
                kind: "mono".to_owned(),
                len: array.len(),
                index: 0,
            }))
        }
    }

    /// Returns the deserialized data for the specified component, assuming a mono-batch.
    ///
    /// Returns an error if the data cannot be deserialized, or if either the row index is out of bounds,
    /// or the underlying batch is not of unit length.
    #[inline]
    pub fn component_mono<C: Component>(&self, row_index: usize) -> Option<ChunkResult<C>> {
        let res = self.component_mono_raw(&C::name(), row_index)?;

        let array = match res {
            Ok(array) => array,
            Err(err) => return Some(Err(err)),
        };

        match C::from_arrow(&*array) {
            Ok(data) => data.into_iter().next().map(Ok), // NOTE: It's already sliced!
            Err(err) => Some(Err(err.into())),
        }
    }
}

// --- Unit ---

/// A simple type alias for an `Arc<Chunk>`.
pub type ChunkShared = Arc<Chunk>;

/// A [`ChunkShared`] that is guaranteed to always contain a single row's worth of data.
#[derive(Debug, Clone)]
pub struct UnitChunkShared(ChunkShared);

impl std::ops::Deref for UnitChunkShared {
    type Target = Chunk;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SizeBytes for UnitChunkShared {
    #[inline]
    fn heap_size_bytes(&self) -> u64 {
        Chunk::heap_size_bytes(&self.0)
    }
}

impl Chunk {
    /// Turns the chunk into a [`UnitChunkShared`], if possible.
    #[inline]
    pub fn to_unit(self: &ChunkShared) -> Option<UnitChunkShared> {
        (self.num_rows() == 1).then(|| UnitChunkShared(Arc::clone(self)))
    }

    /// Turns the chunk into a [`UnitChunkShared`], if possible.
    #[inline]
    pub fn into_unit(self) -> Option<UnitChunkShared> {
        (self.num_rows() == 1).then(|| UnitChunkShared(Arc::new(self)))
    }
}

impl UnitChunkShared {
    // Turns the unit chunk back into a standard [`Chunk`].
    #[inline]
    pub fn into_chunk(self) -> ChunkShared {
        self.0
    }
}

impl UnitChunkShared {
    /// Returns the index (`(TimeInt, RowId)` pair) of the single row within, on the given timeline.
    ///
    /// Returns the single static index if the chunk is static.
    #[inline]
    pub fn index(&self, timeline: &Timeline) -> Option<(TimeInt, RowId)> {
        debug_assert!(self.num_rows() == 1);
        if self.is_static() {
            self.row_ids()
                .next()
                .map(|row_id| (TimeInt::STATIC, row_id))
        } else {
            self.timelines.get(timeline).and_then(|time_chunk| {
                time_chunk
                    .times()
                    .next()
                    .and_then(|time| self.row_ids().next().map(|row_id| (time, row_id)))
            })
        }
    }

    /// Returns the [`RowId`] of the single row within, on the given timeline.
    ///
    /// Returns the single static `RowId` if the chunk is static.
    #[inline]
    pub fn row_id(&self) -> Option<RowId> {
        debug_assert!(self.num_rows() == 1);
        self.row_ids().next()
    }

    /// Returns the number of instances of the single row within.
    ///
    /// The maximum value amongst all components is what's returned.
    #[inline]
    pub fn num_instances(&self) -> u64 {
        debug_assert!(self.num_rows() == 1);
        self.components
            .values()
            .map(|list_array| {
                let array = list_array.value(0);
                array.validity().map_or_else(
                    || array.len(),
                    |validity| validity.len() - validity.unset_bits(),
                )
            })
            .max()
            .unwrap_or(0) as u64
    }
}

// --- Unit helpers ---

impl UnitChunkShared {
    // --- Batch ---

    /// Returns the raw data for the specified component.
    #[inline]
    pub fn component_batch_raw(
        &self,
        component_name: &ComponentName,
    ) -> Option<Box<dyn ArrowArray>> {
        debug_assert!(self.num_rows() == 1);
        self.components
            .get(component_name)
            .map(|list_array| list_array.value(0))
    }

    /// Returns the deserialized data for the specified component.
    ///
    /// Returns an error if the data cannot be deserialized.
    #[inline]
    pub fn component_batch<C: Component>(&self) -> Option<ChunkResult<Vec<C>>> {
        let data = C::from_arrow(&*self.component_batch_raw(&C::name())?);
        Some(data.map_err(Into::into))
    }

    // --- Instance ---

    /// Returns the raw data for the specified component at the given instance index.
    ///
    /// Returns an error if the instance index is out of bounds.
    #[inline]
    pub fn component_instance_raw(
        &self,
        component_name: &ComponentName,
        instance_index: usize,
    ) -> Option<ChunkResult<Box<dyn ArrowArray>>> {
        let array = self.component_batch_raw(component_name)?;
        if array.len() > instance_index {
            Some(Ok(array.sliced(instance_index, 1)))
        } else {
            Some(Err(crate::ChunkError::IndexOutOfBounds {
                kind: "instance".to_owned(),
                len: array.len(),
                index: instance_index,
            }))
        }
    }

    /// Returns the deserialized data for the specified component at the given instance index.
    ///
    /// Returns an error if the data cannot be deserialized, or if the instance index is out of bounds.
    #[inline]
    pub fn component_instance<C: Component>(
        &self,
        instance_index: usize,
    ) -> Option<ChunkResult<C>> {
        let res = self.component_instance_raw(&C::name(), instance_index)?;

        let array = match res {
            Ok(array) => array,
            Err(err) => return Some(Err(err)),
        };

        match C::from_arrow(&*array) {
            Ok(data) => data.into_iter().next().map(Ok), // NOTE: It's already sliced!
            Err(err) => Some(Err(err.into())),
        }
    }

    // --- Mono ---

    /// Returns the raw data for the specified component, assuming a mono-batch.
    ///
    /// Returns an error if the underlying batch is not of unit length.
    #[inline]
    pub fn component_mono_raw(
        &self,
        component_name: &ComponentName,
    ) -> Option<ChunkResult<Box<dyn ArrowArray>>> {
        let array = self.component_batch_raw(component_name)?;
        if array.len() == 1 {
            Some(Ok(array.sliced(0, 1)))
        } else {
            Some(Err(crate::ChunkError::IndexOutOfBounds {
                kind: "mono".to_owned(),
                len: array.len(),
                index: 0,
            }))
        }
    }

    /// Returns the deserialized data for the specified component, assuming a mono-batch.
    ///
    /// Returns an error if the data cannot be deserialized, or if the underlying batch is not of unit length.
    #[inline]
    pub fn component_mono<C: Component>(&self) -> Option<ChunkResult<C>> {
        let res = self.component_mono_raw(&C::name())?;

        let array = match res {
            Ok(array) => array,
            Err(err) => return Some(Err(err)),
        };

        match C::from_arrow(&*array) {
            Ok(data) => data.into_iter().next().map(Ok), // NOTE: It's already sliced!
            Err(err) => Some(Err(err.into())),
        }
    }
}
