use arrow2::array::{Array, ListArray};
use re_log_types::{TimeInt, Timeline};
use re_types_core::{Component, ComponentName};

use crate::{Chunk, ChunkResult, RowId};

// TODO: rename this
// TODO: all these docs are shit

// ---

impl Chunk {
    // TODO
    #[inline]
    pub fn list_array(&self, component_name: &ComponentName) -> Option<&ListArray<i32>> {
        self.components.get(component_name)
    }

    // TODO
    #[inline]
    pub fn list_array_or_empty<C: Component>(&self) -> ListArray<i32> {
        self.list_array(&C::name())
            .cloned()
            .unwrap_or_else(|| ListArray::new_empty(C::arrow_datatype()))
    }

    // TODO
    #[inline]
    pub fn array(
        &self,
        component_name: &ComponentName,
        row_index: usize,
    ) -> Option<Box<dyn Array>> {
        let list_array = self.list_array(component_name)?;

        if row_index >= self.num_rows() {
            return None;
        }

        list_array
            .is_valid(row_index)
            .then(|| list_array.value(row_index))
    }

    // TODO
    #[inline]
    pub fn array_or_empty<C: Component>(&self, row_index: usize) -> Box<dyn Array> {
        self.array(&C::name(), row_index)
            .unwrap_or_else(|| arrow2::array::new_empty_array(C::arrow_datatype()).to_boxed())
    }

    // TODO
    #[inline]
    pub fn sliced_array(
        &self,
        component_name: &ComponentName,
        row_index: usize,
        instance_index: usize,
    ) -> Option<Box<dyn Array>> {
        let list_array = self.list_array(component_name)?;

        if row_index >= self.num_rows() {
            return None;
        }

        let array = list_array
            .is_valid(row_index)
            .then(|| list_array.value(row_index))?;

        (instance_index <= array.len()).then(|| array.sliced(instance_index, 1))
    }

    // TODO
    #[inline]
    pub fn sliced_array_or_empty<C: Component>(
        &self,
        row_index: usize,
        instance_index: usize,
    ) -> Box<dyn Array> {
        self.sliced_array(&C::name(), row_index, instance_index)
            .unwrap_or_else(|| arrow2::array::new_empty_array(C::arrow_datatype()).to_boxed())
    }
}

impl Chunk {
    // TODO: question is how does one exposes a nice way of deserializing a whole column?
    // at first i guess we dont care -- we just want views to be able to go straight to the arrow
    // data when performance matters.

    /// Returns the component data as a dense vector.
    ///
    /// Logs a warning and returns `None` if the component cannot be deserialized.
    #[inline]
    pub fn component_batch<C: Component>(&self, row_index: usize) -> Option<Vec<C>> {
        let component_name = C::name();
        let level = re_log::Level::Warn;

        match C::from_arrow(&*self.array(&C::name(), row_index)?) {
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
    pub fn component_batch_or_empty<C: Component>(&self, row_index: usize) -> Vec<C> {
        self.component_batch::<C>(row_index).unwrap_or_default()
    }

    /// Returns the component data of the specified instance.
    ///
    /// Logs a warning and returns `None` if the component cannot be deserialized, or
    /// the index doesn't exist.
    #[inline]
    pub fn component_instance<C: Component>(
        &self,
        row_index: usize,
        instance_index: usize,
    ) -> Option<C> {
        let component_name = C::name();
        let level = re_log::Level::Warn;

        let array = self.component_instance_raw(&component_name, row_index, instance_index)?;
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

    /// Returns the component data of the specified instance.
    ///
    /// Logs a warning and returns `None` if the component cannot be deserialized, or
    /// the index doesn't exist.
    #[inline]
    pub fn component_instance_raw(
        &self,
        component_name: &ComponentName,
        row_index: usize,
        instance_index: usize,
    ) -> Option<Box<dyn Array>> {
        let array = self.array(component_name, row_index)?;

        // TODO(#5259): Figure out if/how we'd like to integrate clamping semantics into the
        // selection panel.
        //
        // For now, we simply always clamp, which is the closest to the legacy behavior that the UI
        // expects.
        let index = usize::min(instance_index, array.len().saturating_sub(1));

        self.sliced_array(component_name, row_index, index)
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
    pub fn component_mono<C: Component>(&self, row_index: usize) -> Option<C> {
        self.component_instance::<C>(row_index, 0)
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
        row_index: usize,
        instance_index: usize,
    ) -> Option<Box<dyn Array>> {
        self.component_instance_raw(component_name, row_index, instance_index)
    }
}
