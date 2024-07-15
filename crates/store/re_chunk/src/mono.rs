use std::sync::Arc;

use arrow2::array::{
    Array as ArrowArray, ListArray as ArrowListArray, PrimitiveArray as ArrowPrimitiveArray,
    StructArray as ArrowStructArray,
};

use re_log_types::{TimeInt, Timeline};
use re_types_core::{Component, ComponentName};

use crate::{Chunk, RowId};

// ---

// TODO
pub type ChunkShared = Arc<Chunk>;

// TODO: `Mono` is already taken to mean "mono instance", not "mono row"...
#[derive(Debug, Clone)]
pub struct ChunkSharedMono(Arc<Chunk>);

impl std::ops::Deref for ChunkSharedMono {
    type Target = Chunk;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Chunk {
    // TODO
    #[inline]
    pub fn to_mono(self: &ChunkShared) -> Option<ChunkSharedMono> {
        (self.num_rows() == 1).then(|| ChunkSharedMono(Arc::clone(self)))
    }

    // TODO
    #[inline]
    pub fn into_mono(self) -> Option<ChunkSharedMono> {
        (self.num_rows() == 1).then(|| ChunkSharedMono(Arc::new(self)))
    }
}

impl ChunkSharedMono {
    #[inline]
    pub fn index(&self, timeline: &Timeline) -> Option<(TimeInt, RowId)> {
        debug_assert!(self.num_rows() == 1);
        self.timelines.get(timeline).and_then(|time_chunk| {
            time_chunk
                .times()
                .next()
                .and_then(|time| self.row_ids().next().map(|row_id| (time, row_id)))
        })
    }

    #[inline]
    pub fn num_instances(&self) -> u64 {
        self.components
            .values()
            .map(|list_array| {
                list_array.validity().map_or_else(
                    || list_array.len(),
                    |validity| validity.len() - validity.unset_bits(),
                )
            })
            .max()
            .unwrap_or(0) as u64
    }
}

// ---

// TODO: Though really there's no reason not to implement this on Chunk, right?

// impl ChunkSharedMono {
//     // TODO
//     #[inline]
//     pub fn list_array(&self, component_name: &ComponentName) -> Option<&ArrowListArray<i32>> {
//         self.components.get(component_name)
//     }
//
//     // TODO
//     #[inline]
//     pub fn list_array_or_empty<C: Component>(&self) -> ArrowListArray<i32> {
//         self.list_array(&C::name())
//             .cloned()
//             .unwrap_or_else(|| ArrowListArray::new_empty(C::arrow_datatype()))
//     }
//
//     // TODO
//     #[inline]
//     pub fn array(
//         &self,
//         component_name: &ComponentName,
//         row_index: usize,
//     ) -> Option<Box<dyn ArrowArray>> {
//         let list_array = self.list_array(component_name)?;
//
//         if row_index >= self.num_rows() {
//             return None;
//         }
//
//         list_array
//             .is_valid(row_index)
//             .then(|| list_array.value(row_index))
//     }
//
//     // TODO
//     #[inline]
//     pub fn array_or_empty<C: Component>(&self, row_index: usize) -> Box<dyn ArrowArray> {
//         self.array(&C::name(), row_index)
//             .unwrap_or_else(|| arrow2::array::new_empty_array(C::arrow_datatype()).to_boxed())
//     }
//
//     // TODO
//     #[inline]
//     pub fn sliced_array(
//         &self,
//         component_name: &ComponentName,
//         row_index: usize,
//         instance_index: usize,
//     ) -> Option<Box<dyn ArrowArray>> {
//         let list_array = self.list_array(component_name)?;
//
//         if row_index >= self.num_rows() {
//             return None;
//         }
//
//         let array = list_array
//             .is_valid(row_index)
//             .then(|| list_array.value(row_index))?;
//
//         (instance_index <= array.len()).then(|| array.sliced(instance_index, 1))
//     }
//
//     // TODO
//     #[inline]
//     pub fn sliced_array_or_empty<C: Component>(
//         &self,
//         row_index: usize,
//         instance_index: usize,
//     ) -> Box<dyn ArrowArray> {
//         self.sliced_array(&C::name(), row_index, instance_index)
//             .unwrap_or_else(|| arrow2::array::new_empty_array(C::arrow_datatype()).to_boxed())
//     }
// }

// TODO
impl ChunkSharedMono {
    //     // TODO
    //     #[inline]
    //     pub fn list_array(&self, component_name: &ComponentName) -> Option<&ArrowListArray<i32>> {
    //         self.components.get(component_name)
    //     }
    //
    //     // TODO
    //     #[inline]
    //     pub fn list_array_or_empty<C: Component>(&self) -> ArrowListArray<i32> {
    //         self.list_array(&C::name())
    //             .cloned()
    //             .unwrap_or_else(|| ArrowListArray::new_empty(C::arrow_datatype()))
    //     }

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

    /// Returns the component data of the specified instance.
    ///
    /// Logs a warning and returns `None` if the component cannot be deserialized, or
    /// the index doesn't exist.
    #[inline]
    pub fn component_instance_raw(
        &self,
        component_name: &ComponentName,
        instance_index: usize,
    ) -> Option<Box<dyn ArrowArray>> {
        let array = self.component_batch_raw(component_name)?;

        // TODO: that is definitely not the job of this layer to do clamping and the like.
        //
        // // TODO(#5259): Figure out if/how we'd like to integrate clamping semantics into the
        // // selection panel.
        // //
        // // For now, we simply always clamp, which is the closest to the legacy behavior that the UI
        // // expects.
        // let index = usize::min(instance_index, array.len().saturating_sub(1));

        (array.len() > instance_index).then(|| array.sliced(instance_index, 1))
    }

    /// Returns the component data of the single instance as an arrow array.
    ///
    /// This assumes that the row we get from the store contains at most one instance for this
    /// component; it will log a warning otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// Logs a warning and returns `None` if the component cannot be deserialized.
    #[inline]
    pub fn component_mono_raw(
        &self,
        component_name: &ComponentName,
    ) -> Option<Box<dyn ArrowArray>> {
        self.component_instance_raw(component_name, 0)
    }

    // TODO: question is how does one exposes a nice way of deserializing a whole column?
    // at first i guess we dont care -- we just want views to be able to go straight to the arrow
    // data when performance matters.

    /// Returns the component data as a dense vector.
    ///
    /// Logs a warning and returns `None` if the component cannot be deserialized.
    #[inline]
    pub fn component_batch<C: Component>(&self) -> Option<Vec<C>> {
        let component_name = C::name();
        let level = re_log::Level::Warn;

        match C::from_arrow(&*self.component_batch_raw(&C::name())?) {
            Ok(data) => Some(data),

            Err(err) => {
                re_log::log_once!(
                    level,
                    "Couldn't deserialize {component_name}: {}",
                    re_error::format_ref(&err),
                );
                None
            }
        }
    }

    #[inline]
    pub fn component_batch_or_empty<C: Component>(&self) -> Vec<C> {
        self.component_batch::<C>().unwrap_or_default()
    }

    /// Returns the component data of the specified instance.
    ///
    /// Logs a warning and returns `None` if the component cannot be deserialized, or
    /// the index doesn't exist.
    #[inline]
    pub fn component_instance<C: Component>(&self, instance_index: usize) -> Option<C> {
        let component_name = C::name();
        let level = re_log::Level::Warn;

        let array = self.component_instance_raw(&component_name, instance_index)?;
        let batch = match C::from_arrow(&*array) {
            Ok(data) => Some(data),

            Err(err) => {
                re_log::log_once!(
                    level,
                    "Couldn't deserialize {component_name}: {}",
                    re_error::format_ref(&err),
                );
                None
            }
        }?;

        match batch.len() {
            0 => {
                None // Empty list = no data.
            }

            1 => Some(batch[0].clone()),

            _len => {
                re_log::log_once!(
                    level,
                    "Couldn't deserialize {component_name}: instance index OOB"
                );
                None
            }
        }
    }

    /// Returns the component data of the single instance.
    ///
    /// This assumes that the row we get from the store contains at most one instance for this
    /// component; it will log a warning otherwise.
    ///
    /// This should only be used for "mono-components" such as `Transform` and `Tensor`.
    ///
    /// Logs a warning and returns `None` if the component cannot be deserialized.
    #[inline]
    pub fn component_mono<C: Component>(&self) -> Option<C> {
        self.component_instance::<C>(0)
    }
}
