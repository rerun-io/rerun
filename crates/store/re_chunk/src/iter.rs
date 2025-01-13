use std::sync::Arc;

use arrow2::{
    array::{
        Array as Arrow2Array, BooleanArray as Arrow2BooleanArray,
        FixedSizeListArray as Arrow2FixedSizeListArray, ListArray as Arrow2ListArray,
        PrimitiveArray as Arrow2PrimitiveArray, StructArray as Arrow2StructArray,
        Utf8Array as Arrow2Utf8Array,
    },
    bitmap::Bitmap as Arrow2Bitmap,
    Either,
};
use itertools::{izip, Itertools};

use re_log_types::{TimeInt, TimePoint, Timeline};
use re_types_core::{ArrowBuffer, ArrowString, Component, ComponentName};

use crate::{Chunk, RowId, TimeColumn};

// ---

// NOTE: Regarding the use of (recursive) `Either` in this file: it is _not_ arbitrary.
//
// They _should_ all follow this model:
// * The first layer is always the emptiness layer: `Left` is empty, `Right` is non-empty.
// * The second layer is the temporarily layer: `Left` is static, `Right` is temporal.
// * Any layers beyond that follow the same pattern: `Left` doesn't have something, while `Right` does.

impl Chunk {
    /// Returns an iterator over the indices (`(TimeInt, RowId)`) of a [`Chunk`], for a given timeline.
    ///
    /// If the chunk is static, `timeline` will be ignored.
    ///
    /// See also:
    /// * [`Self::iter_component_indices`].
    /// * [`Self::iter_indices_owned`].
    #[inline]
    pub fn iter_indices(&self, timeline: &Timeline) -> impl Iterator<Item = (TimeInt, RowId)> + '_ {
        if self.is_static() {
            Either::Right(Either::Left(izip!(
                std::iter::repeat(TimeInt::STATIC),
                self.row_ids()
            )))
        } else {
            let Some(time_column) = self.timelines.get(timeline) else {
                return Either::Left(std::iter::empty());
            };

            Either::Right(Either::Right(izip!(time_column.times(), self.row_ids())))
        }
    }

    /// Returns an iterator over the indices (`(TimeInt, RowId)`) of a [`Chunk`], for a given
    /// timeline and component.
    ///
    /// If the chunk is static, `timeline` will be ignored.
    ///
    /// This is different than [`Self::iter_indices`] in that it will only yield indices for rows
    /// at which there is data for the specified `component_name`.
    ///
    /// See also [`Self::iter_indices`].
    pub fn iter_component_indices(
        &self,
        timeline: &Timeline,
        component_name: &ComponentName,
    ) -> impl Iterator<Item = (TimeInt, RowId)> + '_ {
        let Some(list_array) = self.get_first_component(component_name) else {
            return Either::Left(std::iter::empty());
        };

        if self.is_static() {
            let indices = izip!(std::iter::repeat(TimeInt::STATIC), self.row_ids());

            if let Some(validity) = list_array.validity() {
                Either::Right(Either::Left(Either::Left(
                    indices
                        .enumerate()
                        .filter_map(|(i, o)| validity.get_bit(i).then_some(o)),
                )))
            } else {
                Either::Right(Either::Left(Either::Right(indices)))
            }
        } else {
            let Some(time_column) = self.timelines.get(timeline) else {
                return Either::Left(std::iter::empty());
            };

            let indices = izip!(time_column.times(), self.row_ids());

            if let Some(validity) = list_array.validity() {
                Either::Right(Either::Right(Either::Left(
                    indices
                        .enumerate()
                        .filter_map(|(i, o)| validity.get_bit(i).then_some(o)),
                )))
            } else {
                Either::Right(Either::Right(Either::Right(indices)))
            }
        }
    }

    /// Returns an iterator over the [`TimePoint`]s of a [`Chunk`].
    ///
    /// See also:
    /// * [`Self::iter_component_timepoints`].
    #[inline]
    pub fn iter_timepoints(&self) -> impl Iterator<Item = TimePoint> + '_ {
        let mut timelines = self
            .timelines
            .values()
            .map(|time_column| (time_column.timeline, time_column.times()))
            .collect_vec();

        std::iter::from_fn(move || {
            let mut timepoint = TimePoint::default();
            for (timeline, times) in &mut timelines {
                timepoint.insert(*timeline, times.next()?);
            }
            Some(timepoint)
        })
    }

    /// Returns an iterator over the [`TimePoint`]s of a [`Chunk`], for a given component.
    ///
    /// This is different than [`Self::iter_timepoints`] in that it will only yield timepoints for rows
    /// at which there is data for the specified `component_name`.
    ///
    /// See also [`Self::iter_timepoints`].
    pub fn iter_component_timepoints(
        &self,
        component_name: &ComponentName,
    ) -> impl Iterator<Item = TimePoint> + '_ {
        let Some(list_array) = self.get_first_component(component_name) else {
            return Either::Left(std::iter::empty());
        };

        if let Some(validity) = list_array.validity() {
            let mut timelines = self
                .timelines
                .values()
                .map(|time_column| {
                    (
                        time_column.timeline,
                        time_column
                            .times()
                            .enumerate()
                            .filter(|(i, _)| validity.get_bit(*i))
                            .map(|(_, time)| time),
                    )
                })
                .collect_vec();

            Either::Right(Either::Left(std::iter::from_fn(move || {
                let mut timepoint = TimePoint::default();
                for (timeline, times) in &mut timelines {
                    timepoint.insert(*timeline, times.next()?);
                }
                Some(timepoint)
            })))
        } else {
            let mut timelines = self
                .timelines
                .values()
                .map(|time_column| (time_column.timeline, time_column.times()))
                .collect_vec();

            Either::Right(Either::Right(std::iter::from_fn(move || {
                let mut timepoint = TimePoint::default();
                for (timeline, times) in &mut timelines {
                    timepoint.insert(*timeline, times.next()?);
                }
                Some(timepoint)
            })))
        }
    }

    /// Returns an iterator over the offsets (`(offset, len)`) of a [`Chunk`], for a given
    /// component.
    ///
    /// I.e. each `(offset, len)` pair describes the position of a component batch in the
    /// underlying arrow array of values.
    pub fn iter_component_offsets(
        &self,
        component_name: &ComponentName,
    ) -> impl Iterator<Item = (usize, usize)> + '_ {
        let Some(list_array) = self.get_first_component(component_name) else {
            return Either::Left(std::iter::empty());
        };

        let offsets = list_array.offsets().iter().map(|idx| *idx as usize);
        let lengths = list_array.offsets().lengths();

        if let Some(validity) = list_array.validity() {
            Either::Right(Either::Left(
                izip!(offsets, lengths)
                    .enumerate()
                    .filter_map(|(i, o)| validity.get_bit(i).then_some(o)),
            ))
        } else {
            Either::Right(Either::Right(izip!(offsets, lengths)))
        }
    }

    /// Returns an iterator over the all the sliced component batches in a [`Chunk`]'s column, for
    /// a given component.
    ///
    /// The generic `S` parameter will decide the type of data returned. It is _very_ permissive.
    /// See [`ChunkComponentSlicer`] for all the available implementations.
    ///
    /// This is a very fast path: the entire column will be downcasted at once, and then every
    /// component batch will be a slice reference into that global slice.
    ///
    /// See also [`Self::iter_slices_from_struct_field`].
    #[inline]
    pub fn iter_slices<'a, S: 'a + ChunkComponentSlicer>(
        &'a self,
        component_name: ComponentName,
    ) -> impl Iterator<Item = S::Item<'a>> + 'a {
        let Some(list_array) = self.get_first_component(&component_name) else {
            return Either::Left(std::iter::empty());
        };

        Either::Right(S::slice(
            component_name,
            &**list_array.values() as _,
            self.iter_component_offsets(&component_name),
        ))
    }

    /// Returns an iterator over the all the sliced component batches in a [`Chunk`]'s column, for
    /// a specific struct field of given component.
    ///
    /// The target component must be a `StructArray`.
    ///
    /// The generic `S` parameter will decide the type of data returned. It is _very_ permissive.
    /// See [`ChunkComponentSlicer`] for all the available implementations.
    ///
    /// This is a very fast path: the entire column will be downcasted at once, and then every
    /// component batch will be a slice reference into that global slice.
    ///
    /// See also [`Self::iter_slices_from_struct_field`].
    pub fn iter_slices_from_struct_field<'a, S: 'a + ChunkComponentSlicer>(
        &'a self,
        component_name: ComponentName,
        field_name: &'a str,
    ) -> impl Iterator<Item = S::Item<'a>> + '_ {
        let Some(list_array) = self.get_first_component(&component_name) else {
            return Either::Left(std::iter::empty());
        };

        let Some(struct_array) = list_array
            .values()
            .as_any()
            .downcast_ref::<Arrow2StructArray>()
        else {
            if cfg!(debug_assertions) {
                panic!("downcast failed for {component_name}, data discarded");
            } else {
                re_log::error_once!("downcast failed for {component_name}, data discarded");
            }
            return Either::Left(std::iter::empty());
        };

        let Some(field_idx) = struct_array
            .fields()
            .iter()
            .enumerate()
            .find_map(|(i, field)| (field.name == field_name).then_some(i))
        else {
            if cfg!(debug_assertions) {
                panic!("field {field_name} not found for {component_name}, data discarded");
            } else {
                re_log::error_once!(
                    "field {field_name} not found for {component_name}, data discarded"
                );
            }
            return Either::Left(std::iter::empty());
        };

        let Some(array) = struct_array.values().get(field_idx) else {
            if cfg!(debug_assertions) {
                panic!("field {field_name} not found for {component_name}, data discarded");
            } else {
                re_log::error_once!(
                    "field {field_name} not found for {component_name}, data discarded"
                );
            }
            return Either::Left(std::iter::empty());
        };

        Either::Right(S::slice(
            component_name,
            &**array,
            self.iter_component_offsets(&component_name),
        ))
    }
}

// ---

/// A `ChunkComponentSlicer` knows how to efficiently slice component batches out of a Chunk column.
///
/// See [`Chunk::iter_slices`] and [`Chunk::iter_slices_from_struct_field`].
pub trait ChunkComponentSlicer {
    type Item<'a>;

    fn slice<'a>(
        component_name: ComponentName,
        array: &'a dyn Arrow2Array,
        component_offsets: impl Iterator<Item = (usize, usize)> + 'a,
    ) -> impl Iterator<Item = Self::Item<'a>> + 'a;
}

/// The actual implementation of `impl_native_type!`, so that we don't have to work in a macro.
fn slice_as_native<'a, T: arrow2::types::NativeType + arrow::datatypes::ArrowNativeType>(
    component_name: ComponentName,
    array: &'a dyn Arrow2Array,
    component_offsets: impl Iterator<Item = (usize, usize)> + 'a,
) -> impl Iterator<Item = &'a [T]> + 'a {
    let Some(values) = array.as_any().downcast_ref::<Arrow2PrimitiveArray<T>>() else {
        if cfg!(debug_assertions) {
            panic!("downcast failed for {component_name}, data discarded");
        } else {
            re_log::error_once!("downcast failed for {component_name}, data discarded");
        }
        return Either::Left(std::iter::empty());
    };
    let values = values.values().as_slice();

    // NOTE: No need for validity checks here, `iter_offsets` already takes care of that.
    Either::Right(component_offsets.map(move |(idx, len)| &values[idx..idx + len]))
}

// We use a macro instead of a blanket impl because this violates orphan rules.
macro_rules! impl_native_type {
    ($type:ty) => {
        impl ChunkComponentSlicer for $type {
            type Item<'a> = &'a [$type];

            fn slice<'a>(
                component_name: ComponentName,
                array: &'a dyn Arrow2Array,
                component_offsets: impl Iterator<Item = (usize, usize)> + 'a,
            ) -> impl Iterator<Item = Self::Item<'a>> + 'a {
                slice_as_native(component_name, array, component_offsets)
            }
        }
    };
}

impl_native_type!(u8);
impl_native_type!(u16);
impl_native_type!(u32);
impl_native_type!(u64);
impl_native_type!(i8);
impl_native_type!(i16);
impl_native_type!(i32);
impl_native_type!(i64);
impl_native_type!(f32);
impl_native_type!(f64);
impl_native_type!(i128);

/// The actual implementation of `impl_array_native_type!`, so that we don't have to work in a macro.
fn slice_as_array_native<
    'a,
    const N: usize,
    T: arrow2::types::NativeType + arrow::datatypes::ArrowNativeType,
>(
    component_name: ComponentName,
    array: &'a dyn Arrow2Array,
    component_offsets: impl Iterator<Item = (usize, usize)> + 'a,
) -> impl Iterator<Item = &'a [[T; N]]> + 'a
where
    [T; N]: bytemuck::Pod,
{
    let Some(fixed_size_list_array) = array.as_any().downcast_ref::<Arrow2FixedSizeListArray>()
    else {
        if cfg!(debug_assertions) {
            panic!("downcast failed for {component_name}, data discarded");
        } else {
            re_log::error_once!("downcast failed for {component_name}, data discarded");
        }
        return Either::Left(std::iter::empty());
    };

    let Some(values) = fixed_size_list_array
        .values()
        .as_any()
        .downcast_ref::<Arrow2PrimitiveArray<T>>()
    else {
        if cfg!(debug_assertions) {
            panic!("downcast failed for {component_name}, data discarded");
        } else {
            re_log::error_once!("downcast failed for {component_name}, data discarded");
        }
        return Either::Left(std::iter::empty());
    };

    let size = fixed_size_list_array.size();
    let values = values.values().as_slice();

    // NOTE: No need for validity checks here, `component_offsets` already takes care of that.
    Either::Right(
        component_offsets.map(move |(idx, len)| {
            bytemuck::cast_slice(&values[idx * size..idx * size + len * size])
        }),
    )
}

// We use a macro instead of a blanket impl because this violates orphan rules.
macro_rules! impl_array_native_type {
    ($type:ty) => {
        impl<const N: usize> ChunkComponentSlicer for [$type; N]
        where
            [$type; N]: bytemuck::Pod,
        {
            type Item<'a> = &'a [[$type; N]];

            fn slice<'a>(
                component_name: ComponentName,
                array: &'a dyn Arrow2Array,
                component_offsets: impl Iterator<Item = (usize, usize)> + 'a,
            ) -> impl Iterator<Item = Self::Item<'a>> + 'a {
                slice_as_array_native(component_name, array, component_offsets)
            }
        }
    };
}

impl_array_native_type!(u8);
impl_array_native_type!(u16);
impl_array_native_type!(u32);
impl_array_native_type!(u64);
impl_array_native_type!(i8);
impl_array_native_type!(i16);
impl_array_native_type!(i32);
impl_array_native_type!(i64);
impl_array_native_type!(f32);
impl_array_native_type!(f64);
impl_array_native_type!(i128);

/// The actual implementation of `impl_buffer_native_type!`, so that we don't have to work in a macro.
fn slice_as_buffer_native<'a, T: arrow2::types::NativeType + arrow::datatypes::ArrowNativeType>(
    component_name: ComponentName,
    array: &'a dyn Arrow2Array,
    component_offsets: impl Iterator<Item = (usize, usize)> + 'a,
) -> impl Iterator<Item = Vec<ArrowBuffer<T>>> + 'a {
    let Some(inner_list_array) = array.as_any().downcast_ref::<Arrow2ListArray<i32>>() else {
        if cfg!(debug_assertions) {
            panic!("downcast failed for {component_name}, data discarded");
        } else {
            re_log::error_once!("downcast failed for {component_name}, data discarded");
        }
        return Either::Left(std::iter::empty());
    };

    let Some(values) = inner_list_array
        .values()
        .as_any()
        .downcast_ref::<Arrow2PrimitiveArray<T>>()
    else {
        if cfg!(debug_assertions) {
            panic!("downcast failed for {component_name}, data discarded");
        } else {
            re_log::error_once!("downcast failed for {component_name}, data discarded");
        }
        return Either::Left(std::iter::empty());
    };

    let values = values.values();
    let offsets = inner_list_array.offsets();
    let lengths = inner_list_array.offsets().lengths().collect_vec();

    // NOTE: No need for validity checks here, `component_offsets` already takes care of that.
    Either::Right(component_offsets.map(move |(idx, len)| {
        let offsets = &offsets.as_slice()[idx..idx + len];
        let lengths = &lengths.as_slice()[idx..idx + len];
        izip!(offsets, lengths)
            // NOTE: Not an actual clone, just a refbump of the underlying buffer.
            .map(|(&idx, &len)| values.clone().sliced(idx as _, len).into())
            .collect_vec()
    }))
}

// We use a macro instead of a blanket impl because this violates orphan rules.
macro_rules! impl_buffer_native_type {
    ($type:ty) => {
        impl ChunkComponentSlicer for &[$type] {
            type Item<'a> = Vec<ArrowBuffer<$type>>;

            fn slice<'a>(
                component_name: ComponentName,
                array: &'a dyn Arrow2Array,
                component_offsets: impl Iterator<Item = (usize, usize)> + 'a,
            ) -> impl Iterator<Item = Self::Item<'a>> + 'a {
                slice_as_buffer_native(component_name, array, component_offsets)
            }
        }
    };
}

impl_buffer_native_type!(u8);
impl_buffer_native_type!(u16);
impl_buffer_native_type!(u32);
impl_buffer_native_type!(u64);
impl_buffer_native_type!(i8);
impl_buffer_native_type!(i16);
impl_buffer_native_type!(i32);
impl_buffer_native_type!(i64);
impl_buffer_native_type!(f32);
impl_buffer_native_type!(f64);
impl_buffer_native_type!(i128);

/// The actual implementation of `impl_array_list_native_type!`, so that we don't have to work in a macro.
fn slice_as_array_list_native<
    'a,
    const N: usize,
    T: arrow2::types::NativeType + arrow::datatypes::ArrowNativeType,
>(
    component_name: ComponentName,
    array: &'a dyn Arrow2Array,
    component_offsets: impl Iterator<Item = (usize, usize)> + 'a,
) -> impl Iterator<Item = Vec<&'a [[T; N]]>> + 'a
where
    [T; N]: bytemuck::Pod,
{
    let Some(inner_list_array) = array.as_any().downcast_ref::<Arrow2ListArray<i32>>() else {
        if cfg!(debug_assertions) {
            panic!("downcast failed for {component_name}, data discarded");
        } else {
            re_log::error_once!("downcast failed for {component_name}, data discarded");
        }
        return Either::Left(std::iter::empty());
    };

    let inner_offsets = inner_list_array.offsets();
    let inner_lengths = inner_list_array.offsets().lengths().collect_vec();

    let Some(fixed_size_list_array) = inner_list_array
        .values()
        .as_any()
        .downcast_ref::<Arrow2FixedSizeListArray>()
    else {
        if cfg!(debug_assertions) {
            panic!("downcast failed for {component_name}, data discarded");
        } else {
            re_log::error_once!("downcast failed for {component_name}, data discarded");
        }
        return Either::Left(std::iter::empty());
    };

    let Some(values) = fixed_size_list_array
        .values()
        .as_any()
        .downcast_ref::<Arrow2PrimitiveArray<T>>()
    else {
        if cfg!(debug_assertions) {
            panic!("downcast failed for {component_name}, data discarded");
        } else {
            re_log::error_once!("downcast failed for {component_name}, data discarded");
        }
        return Either::Left(std::iter::empty());
    };

    let size = fixed_size_list_array.size();
    let values = values.values();

    // NOTE: No need for validity checks here, `iter_offsets` already takes care of that.
    Either::Right(component_offsets.map(move |(idx, len)| {
        let inner_offsets = &inner_offsets.as_slice()[idx..idx + len];
        let inner_lengths = &inner_lengths.as_slice()[idx..idx + len];
        izip!(inner_offsets, inner_lengths)
            .map(|(&idx, &len)| {
                let idx = idx as usize;
                bytemuck::cast_slice(&values[idx * size..idx * size + len * size])
            })
            .collect_vec()
    }))
}

// We use a macro instead of a blanket impl because this violates orphan rules.
macro_rules! impl_array_list_native_type {
    ($type:ty) => {
        impl<const N: usize> ChunkComponentSlicer for &[[$type; N]]
        where
            [$type; N]: bytemuck::Pod,
        {
            type Item<'a> = Vec<&'a [[$type; N]]>;

            fn slice<'a>(
                component_name: ComponentName,
                array: &'a dyn Arrow2Array,
                component_offsets: impl Iterator<Item = (usize, usize)> + 'a,
            ) -> impl Iterator<Item = Self::Item<'a>> + 'a {
                slice_as_array_list_native(component_name, array, component_offsets)
            }
        }
    };
}

impl_array_list_native_type!(u8);
impl_array_list_native_type!(u16);
impl_array_list_native_type!(u32);
impl_array_list_native_type!(u64);
impl_array_list_native_type!(i8);
impl_array_list_native_type!(i16);
impl_array_list_native_type!(i32);
impl_array_list_native_type!(i64);
impl_array_list_native_type!(f32);
impl_array_list_native_type!(f64);
impl_array_list_native_type!(i128);

impl ChunkComponentSlicer for String {
    type Item<'a> = Vec<ArrowString>;

    fn slice<'a>(
        component_name: ComponentName,
        array: &'a dyn Arrow2Array,
        component_offsets: impl Iterator<Item = (usize, usize)> + 'a,
    ) -> impl Iterator<Item = Vec<ArrowString>> + 'a {
        let Some(utf8_array) = array.as_any().downcast_ref::<Arrow2Utf8Array<i32>>() else {
            if cfg!(debug_assertions) {
                panic!("downcast failed for {component_name}, data discarded");
            } else {
                re_log::error_once!("downcast failed for {component_name}, data discarded");
            }
            return Either::Left(std::iter::empty());
        };

        let values = utf8_array.values();
        let offsets = utf8_array.offsets();
        let lengths = utf8_array.offsets().lengths().collect_vec();

        // NOTE: No need for validity checks here, `component_offsets` already takes care of that.
        Either::Right(component_offsets.map(move |(idx, len)| {
            let offsets = &offsets.as_slice()[idx..idx + len];
            let lengths = &lengths.as_slice()[idx..idx + len];
            izip!(offsets, lengths)
                .map(|(&idx, &len)| ArrowString::from(values.clone().sliced(idx as _, len)))
                .collect_vec()
        }))
    }
}

impl ChunkComponentSlicer for bool {
    type Item<'a> = Arrow2Bitmap;

    fn slice<'a>(
        component_name: ComponentName,
        array: &'a dyn Arrow2Array,
        component_offsets: impl Iterator<Item = (usize, usize)> + 'a,
    ) -> impl Iterator<Item = Self::Item<'a>> + 'a {
        let Some(values) = array.as_any().downcast_ref::<Arrow2BooleanArray>() else {
            if cfg!(debug_assertions) {
                panic!("downcast failed for {component_name}, data discarded");
            } else {
                re_log::error_once!("downcast failed for {component_name}, data discarded");
            }
            return Either::Left(std::iter::empty());
        };
        let values = values.values().clone();

        // NOTE: No need for validity checks here, `component_offsets` already takes care of that.
        Either::Right(component_offsets.map(move |(idx, len)| values.clone().sliced(idx, len)))
    }
}

// ---

pub struct ChunkIndicesIter {
    chunk: Arc<Chunk>,

    time_column: Option<TimeColumn>,
    index: usize,
}

impl Iterator for ChunkIndicesIter {
    type Item = (TimeInt, RowId);

    fn next(&mut self) -> Option<Self::Item> {
        let i = self.index;
        self.index += 1;

        let row_id = {
            let (times, incs) = self.chunk.row_ids_raw();
            let times = times.values();
            let incs = incs.values();

            let time = *times.get(i)?;
            let inc = *incs.get(i)?;

            RowId::from_u128(((time as u128) << 64) | (inc as u128))
        };

        if let Some(time_column) = &self.time_column {
            let time = *time_column.times_raw().get(i)?;
            let time = TimeInt::new_temporal(time);
            Some((time, row_id))
        } else {
            Some((TimeInt::STATIC, row_id))
        }
    }
}

impl Chunk {
    /// Returns an iterator over the indices (`(TimeInt, RowId)`) of a [`Chunk`], for a given timeline.
    ///
    /// If the chunk is static, `timeline` will be ignored.
    ///
    /// The returned iterator outlives `self`, thus it can be passed around freely.
    /// The tradeoff is that `self` must be an `Arc`.
    ///
    /// See also [`Self::iter_indices`].
    #[inline]
    pub fn iter_indices_owned(
        self: Arc<Self>,
        timeline: &Timeline,
    ) -> impl Iterator<Item = (TimeInt, RowId)> {
        if self.is_static() {
            Either::Left(ChunkIndicesIter {
                chunk: self,
                time_column: None,
                index: 0,
            })
        } else {
            self.timelines.get(timeline).cloned().map_or_else(
                || Either::Right(Either::Left(std::iter::empty())),
                |time_column| {
                    Either::Right(Either::Right(ChunkIndicesIter {
                        chunk: self,
                        time_column: Some(time_column),
                        index: 0,
                    }))
                },
            )
        }
    }
}

// ---

/// The actual iterator implementation for [`Chunk::iter_component`].
pub struct ChunkComponentIter<C, IO> {
    values: Arc<Vec<C>>,
    offsets: IO,
}

/// The underlying item type for [`ChunkComponentIter`].
///
/// This allows us to cheaply carry slices of deserialized data, while working around the
/// limitations of Rust's Iterator trait and ecosystem.
///
/// See [`ChunkComponentIterItem::as_slice`].
#[derive(Clone, PartialEq)]
pub struct ChunkComponentIterItem<C> {
    values: Arc<Vec<C>>,
    index: usize,
    len: usize,
}

impl<C: PartialEq> PartialEq<[C]> for ChunkComponentIterItem<C> {
    fn eq(&self, rhs: &[C]) -> bool {
        self.as_slice().eq(rhs)
    }
}

impl<C: PartialEq> PartialEq<Vec<C>> for ChunkComponentIterItem<C> {
    fn eq(&self, rhs: &Vec<C>) -> bool {
        self.as_slice().eq(rhs)
    }
}

impl<C: Eq> Eq for ChunkComponentIterItem<C> {}

// NOTE: No `C: Default`!
impl<C> Default for ChunkComponentIterItem<C> {
    #[inline]
    fn default() -> Self {
        Self {
            values: Arc::new(Vec::new()),
            index: 0,
            len: 0,
        }
    }
}

impl<C> ChunkComponentIterItem<C> {
    #[inline]
    pub fn as_slice(&self) -> &[C] {
        &self.values[self.index..self.index + self.len]
    }
}

impl<C> std::ops::Deref for ChunkComponentIterItem<C> {
    type Target = [C];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<C: Component, IO: Iterator<Item = (usize, usize)>> Iterator for ChunkComponentIter<C, IO> {
    type Item = ChunkComponentIterItem<C>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.offsets
            .next()
            .map(move |(index, len)| ChunkComponentIterItem {
                values: Arc::clone(&self.values),
                index,
                len,
            })
    }
}

impl Chunk {
    /// Returns an iterator over the deserialized batches of a [`Chunk`], for a given component.
    ///
    /// This is a dedicated fast path: the entire column will be downcasted and deserialized at
    /// once, and then every component batch will be a slice reference into that global slice.
    /// Use this when working with complex arrow datatypes and performance matters (e.g. ranging
    /// through enum types across many timestamps).
    ///
    /// TODO(#5305): Note that, while this is much faster than deserializing each row individually,
    /// this still uses the old codegen'd deserialization path, which does some very unidiomatic Arrow
    /// things, and is therefore very slow at the moment. Avoid this on performance critical paths.
    ///
    /// See also:
    /// * [`Self::iter_slices`]
    /// * [`Self::iter_slices_from_struct_field`]
    #[inline]
    pub fn iter_component<C: Component>(
        &self,
    ) -> ChunkComponentIter<C, impl Iterator<Item = (usize, usize)> + '_> {
        let Some(list_array) = self.get_first_component(&C::name()) else {
            return ChunkComponentIter {
                values: Arc::new(vec![]),
                offsets: Either::Left(std::iter::empty()),
            };
        };

        let values = arrow::array::ArrayRef::from(list_array.values().clone());
        let values = match C::from_arrow(&values) {
            Ok(values) => values,
            Err(err) => {
                if cfg!(debug_assertions) {
                    panic!(
                        "[DEBUG-ONLY] deserialization failed for {}, data discarded: {}",
                        C::name(),
                        re_error::format_ref(&err),
                    );
                } else {
                    re_log::error_once!(
                        "deserialization failed for {}, data discarded: {}",
                        C::name(),
                        re_error::format_ref(&err),
                    );
                }
                return ChunkComponentIter {
                    values: Arc::new(vec![]),
                    offsets: Either::Left(std::iter::empty()),
                };
            }
        };

        // NOTE: No need for validity checks here, `iter_offsets` already takes care of that.
        ChunkComponentIter {
            values: Arc::new(values),
            offsets: Either::Right(self.iter_component_offsets(&C::name())),
        }
    }
}

// ---

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use itertools::{izip, Itertools};
    use re_log_types::{example_components::MyPoint, EntityPath, TimeInt, TimePoint};

    use crate::{Chunk, RowId, Timeline};

    #[test]
    fn iter_indices_temporal() -> anyhow::Result<()> {
        let entity_path = EntityPath::from("this/that");

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();

        let timeline_frame = Timeline::new_sequence("frame");

        let timepoint1 = [(timeline_frame, 1)];
        let timepoint2 = [(timeline_frame, 3)];
        let timepoint3 = [(timeline_frame, 5)];
        let timepoint4 = [(timeline_frame, 7)];
        let timepoint5 = [(timeline_frame, 9)];

        let points1 = &[MyPoint::new(1.0, 1.0)];
        let points2 = &[MyPoint::new(2.0, 2.0)];
        let points3 = &[MyPoint::new(3.0, 3.0)];
        let points4 = &[MyPoint::new(4.0, 4.0)];
        let points5 = &[MyPoint::new(5.0, 5.0)];

        let chunk = Arc::new(
            Chunk::builder(entity_path.clone())
                .with_component_batches(row_id1, timepoint1, [points1 as _])
                .with_component_batches(row_id2, timepoint2, [points2 as _])
                .with_component_batches(row_id3, timepoint3, [points3 as _])
                .with_component_batches(row_id4, timepoint4, [points4 as _])
                .with_component_batches(row_id5, timepoint5, [points5 as _])
                .build()?,
        );

        {
            let got = Arc::clone(&chunk)
                .iter_indices_owned(&timeline_frame)
                .collect_vec();
            let expected = izip!(
                chunk
                    .timelines
                    .get(&timeline_frame)
                    .map(|time_column| time_column.times().collect_vec())
                    .unwrap_or_default(),
                chunk.row_ids()
            )
            .collect_vec();

            similar_asserts::assert_eq!(expected, got);
        }

        Ok(())
    }

    #[test]
    fn iter_indices_static() -> anyhow::Result<()> {
        let entity_path = EntityPath::from("this/that");

        let row_id1 = RowId::new();
        let row_id2 = RowId::new();
        let row_id3 = RowId::new();
        let row_id4 = RowId::new();
        let row_id5 = RowId::new();

        let timeline_frame = Timeline::new_sequence("frame");

        let points1 = &[MyPoint::new(1.0, 1.0)];
        let points2 = &[MyPoint::new(2.0, 2.0)];
        let points3 = &[MyPoint::new(3.0, 3.0)];
        let points4 = &[MyPoint::new(4.0, 4.0)];
        let points5 = &[MyPoint::new(5.0, 5.0)];

        let chunk = Arc::new(
            Chunk::builder(entity_path.clone())
                .with_component_batches(row_id1, TimePoint::default(), [points1 as _])
                .with_component_batches(row_id2, TimePoint::default(), [points2 as _])
                .with_component_batches(row_id3, TimePoint::default(), [points3 as _])
                .with_component_batches(row_id4, TimePoint::default(), [points4 as _])
                .with_component_batches(row_id5, TimePoint::default(), [points5 as _])
                .build()?,
        );

        {
            let got = Arc::clone(&chunk)
                .iter_indices_owned(&timeline_frame)
                .collect_vec();
            let expected = izip!(std::iter::repeat(TimeInt::STATIC), chunk.row_ids()).collect_vec();

            similar_asserts::assert_eq!(expected, got);
        }

        Ok(())
    }
}
