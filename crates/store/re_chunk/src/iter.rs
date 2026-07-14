use std::sync::Arc;

use arrow::array::{
    Array as ArrowArray, ArrayRef as ArrowArrayRef, ArrowPrimitiveType, BinaryArray,
    BooleanArray as ArrowBooleanArray, FixedSizeListArray as ArrowFixedSizeListArray,
    GenericStringArray as ArrowGenericStringArray, LargeBinaryArray,
    LargeStringArray as ArrowLargeStringArray, ListArray as ArrowListArray, OffsetSizeTrait,
    PrimitiveArray as ArrowPrimitiveArray, StringArray as ArrowStringArray,
    StructArray as ArrowStructArray,
};
use arrow::buffer::{
    BooleanBuffer as ArrowBooleanBuffer, Buffer, NullBuffer as ArrowNullBuffer,
    ScalarBuffer as ArrowScalarBuffer,
};
use arrow::datatypes::ArrowNativeType;
use itertools::{Either, Itertools as _, izip};
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_log_types::{TimeInt, TimePoint, TimelineName};
use re_span::Span;
use re_types_core::{ArrowString, Component, ComponentIdentifier};

use crate::{Chunk, RowId, TimeColumn};

// ---

fn error_on_downcast_failure(
    component: ComponentIdentifier,
    target: &str,
    actual: &arrow::datatypes::DataType,
) {
    re_log::debug_panic!(
        "downcast to {target} failed for {component}. Array data type was {actual:?}. Data discarded"
    );
    re_log::error_once!(
        "downcast to {target} failed for {component}. Array data type was {actual:?}. Data discarded"
    );
}

// ---

// NOTE: Regarding the use of (recursive) `Either` in this file: it is _not_ arbitrary.
//
// They _should_ all follow this model:
// * The first layer is always the emptiness layer: `Left` is empty, `Right` is non-empty.
// * The second layer is the temporarily layer: `Left` is static, `Right` is temporal.
// * Any layers beyond that follow the same pattern: `Left` doesn't have something, while `Right` does.

impl Chunk {
    /// Return the raw component list array values for a given component.
    ///
    /// Use with great care: Component data may have arbitrary gaps.
    pub fn raw_component_array(&self, component: ComponentIdentifier) -> Option<&ArrowArrayRef> {
        self.components
            .get_array(component)
            .map(|list_array| list_array.values())
    }

    /// Returns an iterator over the indices (`(TimeInt, RowId)`) of a [`Chunk`], for a given timeline.
    ///
    /// If the chunk is static, `timeline` will be ignored.
    ///
    /// See also:
    /// * [`Self::iter_component_indices`].
    /// * [`Self::iter_indices_owned`].
    #[inline]
    pub fn iter_indices(
        &self,
        timeline: &TimelineName,
    ) -> impl Iterator<Item = (TimeInt, RowId)> + '_ + use<'_> {
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
    /// at which there is data for the specified component.
    ///
    /// See also [`Self::iter_indices`].
    pub fn iter_component_indices(
        &self,
        timeline: TimelineName,
        component: ComponentIdentifier,
    ) -> impl Iterator<Item = (TimeInt, RowId)> + '_ + use<'_> {
        let Some(list_array) = self.components.get_array(component) else {
            return Either::Left(std::iter::empty());
        };

        if self.is_static() {
            let indices = izip!(std::iter::repeat(TimeInt::STATIC), self.row_ids());

            if let Some(validity) = list_array.nulls() {
                Either::Right(Either::Left(Either::Left(
                    indices
                        .enumerate()
                        .filter_map(|(i, o)| validity.is_valid(i).then_some(o)),
                )))
            } else {
                Either::Right(Either::Left(Either::Right(indices)))
            }
        } else {
            let Some(time_column) = self.timelines.get(&timeline) else {
                return Either::Left(std::iter::empty());
            };

            let indices = izip!(time_column.times(), self.row_ids());

            if let Some(validity) = list_array.nulls() {
                Either::Right(Either::Right(Either::Left(
                    indices
                        .enumerate()
                        .filter_map(|(i, o)| validity.is_valid(i).then_some(o)),
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
        let timelines = self
            .timelines
            .values()
            .map(|time_column| (time_column.timeline, time_column.times_raw()))
            .collect_vec();

        (0..self.num_rows()).map(move |row| {
            let mut timepoint = TimePoint::default();
            for (timeline, times) in &timelines {
                timepoint.insert(*timeline, TimeInt::new_temporal(times[row]));
            }
            timepoint
        })
    }

    /// Returns an iterator over the [`TimePoint`]s of a [`Chunk`], for a given component.
    ///
    /// This is different than [`Self::iter_timepoints`] in that it will only yield timepoints for rows
    /// at which there is data for the specified component.
    ///
    /// See also [`Self::iter_timepoints`].
    pub fn iter_component_timepoints(
        &self,
        component: ComponentIdentifier,
    ) -> impl Iterator<Item = TimePoint> + '_ + use<'_> {
        let Some(list_array) = self.components.get_array(component) else {
            return Either::Left(std::iter::empty());
        };

        let timelines = self
            .timelines
            .values()
            .map(|time_column| (time_column.timeline, time_column.times_raw()))
            .collect_vec();

        let validity = list_array.nulls();

        Either::Right(
            (0..self.num_rows())
                .filter(move |&row| validity.is_none_or(|validity| validity.is_valid(row)))
                .map(move |row| {
                    let mut timepoint = TimePoint::default();
                    for (timeline, times) in &timelines {
                        timepoint.insert(*timeline, TimeInt::new_temporal(times[row]));
                    }
                    timepoint
                }),
        )
    }

    /// Returns an iterator over the offsets & lengths of component arrays within [`Chunk`], for a given
    /// component.
    ///
    /// I.e. each span describes the position of a component batch in the
    /// underlying arrow array of values.
    pub fn iter_component_offsets(
        &self,
        component: ComponentIdentifier,
    ) -> impl Iterator<Item = Span<usize>> {
        let Some(list_array) = self.components.get_array(component) else {
            return Either::Left(std::iter::empty());
        };

        let offsets = list_array.offsets().iter().map(|idx| *idx as usize);
        let lengths = list_array.offsets().lengths();

        if let Some(validity) = list_array.nulls() {
            Either::Right(Either::Left(
                izip!(offsets, lengths)
                    .enumerate()
                    .filter_map(|(i, o)| validity.is_valid(i).then_some(o))
                    .map(|(start, len)| Span { start, len }),
            ))
        } else {
            Either::Right(Either::Right(
                izip!(offsets, lengths).map(|(start, len)| Span { start, len }),
            ))
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
        component: ComponentIdentifier,
    ) -> impl Iterator<Item = S::Item<'a>> + 'a + use<'a, S> {
        let Some(list_array) = self.components.get_array(component) else {
            return Either::Left(std::iter::empty());
        };

        let component_offset_values = self.iter_component_offsets(component);

        Either::Right(S::slice(
            component,
            &**list_array.values() as _,
            component_offset_values,
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
        component: ComponentIdentifier,
        field_name: &'a str,
    ) -> impl Iterator<Item = S::Item<'a>> + 'a {
        let Some(list_array) = self.components.get_array(component) else {
            return Either::Left(std::iter::empty());
        };

        let Some(struct_array) = list_array.values().downcast_array_ref::<ArrowStructArray>()
        else {
            error_on_downcast_failure(component, "ArrowStructArray", list_array.data_type());
            return Either::Left(std::iter::empty());
        };

        let Some(field_idx) = struct_array
            .fields()
            .iter()
            .enumerate()
            .find_map(|(i, field)| (field.name() == field_name).then_some(i))
        else {
            re_log::debug_panic!("field {field_name} not found for {component}, data discarded");
            re_log::error_once!("field {field_name} not found for {component}, data discarded");
            return Either::Left(std::iter::empty());
        };

        if field_idx >= struct_array.num_columns() {
            re_log::debug_panic!("field {field_name} not found for {component}, data discarded");
            re_log::error_once!("field {field_name} not found for {component}, data discarded");
            return Either::Left(std::iter::empty());
        }

        let component_offset_values = self.iter_component_offsets(component);

        Either::Right(S::slice(
            component,
            struct_array.column(field_idx),
            component_offset_values,
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
        component: ComponentIdentifier,
        array: &'a dyn ArrowArray,
        component_spans: impl Iterator<Item = Span<usize>> + 'a,
    ) -> impl Iterator<Item = Self::Item<'a>>;
}

/// The actual implementation of `impl_native_type!`, so that we don't have to work in a macro.
fn slice_as_native<'a, P, T>(
    component: ComponentIdentifier,
    array: &'a dyn ArrowArray,
    component_spans: impl Iterator<Item = Span<usize>> + 'a,
) -> impl Iterator<Item = &'a [T]> + 'a
where
    P: ArrowPrimitiveType<Native = T>,
    T: ArrowNativeType,
{
    let Some(values) = array.downcast_array_ref::<ArrowPrimitiveArray<P>>() else {
        error_on_downcast_failure(component, "ArrowPrimitiveArray<T>", array.data_type());
        return Either::Left(std::iter::empty());
    };
    let values = values.values().as_ref();

    // NOTE: No need for validity checks here, `iter_offsets` already takes care of that.
    Either::Right(component_spans.map(move |range| &values[range.range()]))
}

// We use a macro instead of a blanket impl because this violates orphan rules.
macro_rules! impl_native_type {
    ($arrow_primitive_type:ty, $native_type:ty) => {
        impl ChunkComponentSlicer for $native_type {
            type Item<'a> = &'a [$native_type];

            fn slice<'a>(
                component: ComponentIdentifier,
                array: &'a dyn ArrowArray,
                component_spans: impl Iterator<Item = Span<usize>> + 'a,
            ) -> impl Iterator<Item = Self::Item<'a>> {
                slice_as_native::<$arrow_primitive_type, $native_type>(
                    component,
                    array,
                    component_spans,
                )
            }
        }
    };
}

impl_native_type!(arrow::array::types::UInt8Type, u8);
impl_native_type!(arrow::array::types::UInt16Type, u16);
impl_native_type!(arrow::array::types::UInt32Type, u32);
impl_native_type!(arrow::array::types::UInt64Type, u64);
// impl_native_type!(arrow::array::types::UInt128Type, u128);
impl_native_type!(arrow::array::types::Int8Type, i8);
impl_native_type!(arrow::array::types::Int16Type, i16);
impl_native_type!(arrow::array::types::Int32Type, i32);
impl_native_type!(arrow::array::types::Int64Type, i64);
// impl_native_type!(arrow::array::types::Int128Type, i128);
impl_native_type!(arrow::array::types::Float16Type, half::f16);
impl_native_type!(arrow::array::types::Float32Type, f32);
impl_native_type!(arrow::array::types::Float64Type, f64);

/// Lazily yields `Option<T>` for one component batch (span) of a primitive column:
/// `None` for null slots, `Some(value)` otherwise.
///
/// Yielded by `slice::<Option<T>>()`, which is like `slice::<T>()` but distinguishes
/// null entries.
pub struct NativeOptSliceIter<'a, T: ArrowNativeType> {
    values: &'a [T],
    nulls: Option<&'a ArrowNullBuffer>,
    range: std::ops::Range<usize>,
}

impl<T: ArrowNativeType> Iterator for NativeOptSliceIter<'_, T> {
    type Item = Option<T>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let i = self.range.next()?;
        Some(if self.nulls.is_some_and(|nulls| !nulls.is_valid(i)) {
            None
        } else {
            Some(self.values[i])
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}

impl<T: ArrowNativeType> ExactSizeIterator for NativeOptSliceIter<'_, T> {}

/// Like [`slice_as_native`] but yields `None` for null entries. Use this variant instead whenever
/// null entries carry meaning (e.g. a state reset).
///
/// The [`slice_as_native`] function reads the raw values buffer without consulting the null
/// bitmap, e.g. `[1.5, null, 2.5]` comes back as `[1.5, 0.0, 2.5]`, silently fabricating a value.
fn slice_as_native_opt<'a, P, T>(
    component: ComponentIdentifier,
    array: &'a dyn ArrowArray,
    component_spans: impl Iterator<Item = Span<usize>> + 'a,
) -> impl Iterator<Item = NativeOptSliceIter<'a, T>> + 'a
where
    P: ArrowPrimitiveType<Native = T>,
    T: ArrowNativeType,
{
    let Some(primitive_array) = array.downcast_array_ref::<ArrowPrimitiveArray<P>>() else {
        error_on_downcast_failure(component, "ArrowPrimitiveArray<T>", array.data_type());
        return Either::Left(std::iter::empty());
    };
    let values = primitive_array.values().as_ref();
    let nulls = primitive_array.nulls();

    Either::Right(component_spans.map(move |span| NativeOptSliceIter {
        values,
        nulls,
        range: span.range(),
    }))
}

// We use a macro instead of a blanket impl because this violates orphan rules.
macro_rules! impl_option_native_type {
    ($arrow_primitive_type:ty, $native_type:ty) => {
        /// Like the plain native slicer but distinguishes null entries: `None` for null,
        /// `Some(value)` otherwise.
        impl ChunkComponentSlicer for Option<$native_type> {
            type Item<'a> = NativeOptSliceIter<'a, $native_type>;

            fn slice<'a>(
                component: ComponentIdentifier,
                array: &'a dyn ArrowArray,
                component_spans: impl Iterator<Item = Span<usize>> + 'a,
            ) -> impl Iterator<Item = Self::Item<'a>> {
                slice_as_native_opt::<$arrow_primitive_type, $native_type>(
                    component,
                    array,
                    component_spans,
                )
            }
        }
    };
}

impl_option_native_type!(arrow::array::types::UInt8Type, u8);
impl_option_native_type!(arrow::array::types::UInt16Type, u16);
impl_option_native_type!(arrow::array::types::UInt32Type, u32);
impl_option_native_type!(arrow::array::types::UInt64Type, u64);
impl_option_native_type!(arrow::array::types::Int8Type, i8);
impl_option_native_type!(arrow::array::types::Int16Type, i16);
impl_option_native_type!(arrow::array::types::Int32Type, i32);
impl_option_native_type!(arrow::array::types::Int64Type, i64);
impl_option_native_type!(arrow::array::types::Float16Type, half::f16);
impl_option_native_type!(arrow::array::types::Float32Type, f32);
impl_option_native_type!(arrow::array::types::Float64Type, f64);

/// The actual implementation of `impl_array_native_type!`, so that we don't have to work in a macro.
fn slice_as_array_native<'a, const N: usize, P, T>(
    component: ComponentIdentifier,
    array: &'a dyn ArrowArray,
    component_spans: impl Iterator<Item = Span<usize>> + 'a,
) -> impl Iterator<Item = &'a [[T; N]]> + 'a
where
    [T; N]: bytemuck::Pod,
    P: ArrowPrimitiveType<Native = T>,
    T: ArrowNativeType + bytemuck::Pod,
{
    let Some(fixed_size_list_array) = array.downcast_array_ref::<ArrowFixedSizeListArray>() else {
        error_on_downcast_failure(component, "ArrowFixedSizeListArray", array.data_type());
        return Either::Left(std::iter::empty());
    };

    let Some(values) = fixed_size_list_array
        .values()
        .downcast_array_ref::<ArrowPrimitiveArray<P>>()
    else {
        error_on_downcast_failure(
            component,
            "ArrowPrimitiveArray<P>",
            fixed_size_list_array.data_type(),
        );
        return Either::Left(std::iter::empty());
    };

    let size = fixed_size_list_array.value_length() as usize;
    let values = values.values().as_ref();

    // NOTE: No need for validity checks here, `component_spans` already takes care of that.
    Either::Right(
        component_spans.map(move |span| bytemuck::cast_slice(&values[(span * size).range()])),
    )
}

// We use a macro instead of a blanket impl because this violates orphan rules.
macro_rules! impl_array_native_type {
    ($arrow_primitive_type:ty, $native_type:ty) => {
        impl<const N: usize> ChunkComponentSlicer for [$native_type; N]
        where
            [$native_type; N]: bytemuck::Pod,
        {
            type Item<'a> = &'a [[$native_type; N]];

            fn slice<'a>(
                component: ComponentIdentifier,
                array: &'a dyn ArrowArray,
                component_spans: impl Iterator<Item = Span<usize>> + 'a,
            ) -> impl Iterator<Item = Self::Item<'a>> {
                slice_as_array_native::<N, $arrow_primitive_type, $native_type>(
                    component,
                    array,
                    component_spans,
                )
            }
        }
    };
}

impl_array_native_type!(arrow::array::types::UInt8Type, u8);
impl_array_native_type!(arrow::array::types::UInt16Type, u16);
impl_array_native_type!(arrow::array::types::UInt32Type, u32);
impl_array_native_type!(arrow::array::types::UInt64Type, u64);
// impl_array_native_type!(arrow::array::types::UInt128Type, u128);
impl_array_native_type!(arrow::array::types::Int8Type, i8);
impl_array_native_type!(arrow::array::types::Int16Type, i16);
impl_array_native_type!(arrow::array::types::Int32Type, i32);
impl_array_native_type!(arrow::array::types::Int64Type, i64);
// impl_array_native_type!(arrow::array::types::Int128Type, i128);
impl_array_native_type!(arrow::array::types::Float16Type, half::f16);
impl_array_native_type!(arrow::array::types::Float32Type, f32);
impl_array_native_type!(arrow::array::types::Float64Type, f64);

/// The actual implementation of `impl_buffer_native_type!`, so that we don't have to work in a macro.
fn slice_as_buffer_native<'a, P, T>(
    component: ComponentIdentifier,
    array: &'a dyn ArrowArray,
    component_spans: impl Iterator<Item = Span<usize>> + 'a,
) -> impl Iterator<Item = Vec<ArrowScalarBuffer<T>>> + 'a
where
    P: ArrowPrimitiveType<Native = T>,
    T: ArrowNativeType,
{
    let Some(inner_list_array) = array.downcast_array_ref::<ArrowListArray>() else {
        error_on_downcast_failure(component, "ArrowListArray", array.data_type());
        return Either::Left(std::iter::empty());
    };

    let Some(values) = inner_list_array
        .values()
        .downcast_array_ref::<ArrowPrimitiveArray<P>>()
    else {
        error_on_downcast_failure(
            component,
            "ArrowPrimitiveArray<P>",
            inner_list_array.data_type(),
        );
        return Either::Left(std::iter::empty());
    };

    let values = values.values();
    let offsets = inner_list_array.offsets();
    let lengths = offsets.lengths().collect_vec();

    // NOTE: No need for validity checks here, `component_spans` already takes care of that.
    Either::Right(component_spans.map(move |span| {
        let offsets = &offsets[span.range()];
        let lengths = &lengths[span.range()];
        izip!(offsets, lengths)
            // NOTE: Not an actual clone, just a refbump of the underlying buffer.
            .map(|(&idx, &len)| values.clone().slice(idx as _, len))
            .collect_vec()
    }))
}

// We special case `&[u8]` so that it works both for `List[u8]` and `Binary/LargeBinary` arrays.
fn slice_as_u8<'a>(
    component: ComponentIdentifier,
    array: &'a dyn ArrowArray,
    component_spans: impl Iterator<Item = Span<usize>> + 'a,
) -> impl Iterator<Item = Vec<Buffer>> + 'a {
    if let Some(binary_array) = array.downcast_array_ref::<BinaryArray>() {
        let values = binary_array.values();
        let offsets = binary_array.offsets();
        let lengths = offsets.lengths().collect_vec();

        // NOTE: No need for validity checks here, `component_spans` already takes care of that.
        Either::Left(Either::Left(component_spans.map(move |span| {
            let offsets = &offsets[span.range()];
            let lengths = &lengths[span.range()];
            izip!(offsets, lengths)
                // NOTE: Not an actual clone, just a refbump of the underlying buffer.
                .map(|(&idx, &len)| values.clone().slice_with_length(idx as _, len))
                .collect_vec()
        })))
    } else if let Some(binary_array) = array.downcast_array_ref::<LargeBinaryArray>() {
        let values = binary_array.values();
        let offsets = binary_array.offsets();
        let lengths = offsets.lengths().collect_vec();

        // NOTE: No need for validity checks here, `component_spans` already takes care of that.
        Either::Left(Either::Right(component_spans.map(move |span| {
            let offsets = &offsets[span.range()];
            let lengths = &lengths[span.range()];
            izip!(offsets, lengths)
                // NOTE: Not an actual clone, just a refbump of the underlying buffer.
                .map(|(&idx, &len)| values.clone().slice_with_length(idx as _, len))
                .collect_vec()
        })))
    } else {
        Either::Right(
            slice_as_buffer_native::<arrow::array::types::UInt8Type, u8>(
                component,
                array,
                component_spans,
            )
            .map(|scalar_buffers| {
                scalar_buffers
                    .into_iter()
                    .map(|scalar_buffer| scalar_buffer.into_inner())
                    .collect_vec()
            }),
        )
    }
}

// We use a macro instead of a blanket impl because this violates orphan rules.
macro_rules! impl_buffer_native_type {
    ($primitive_type:ty, $native_type:ty) => {
        impl ChunkComponentSlicer for &[$native_type] {
            type Item<'a> = Vec<ArrowScalarBuffer<$native_type>>;

            fn slice<'a>(
                component: ComponentIdentifier,
                array: &'a dyn ArrowArray,
                component_spans: impl Iterator<Item = Span<usize>> + 'a,
            ) -> impl Iterator<Item = Self::Item<'a>> {
                slice_as_buffer_native::<$primitive_type, $native_type>(
                    component,
                    array,
                    component_spans,
                )
            }
        }
    };
}

// We special case `&[u8]` so that it works both for `List[u8]` and `Binary` arrays.
impl ChunkComponentSlicer for &[u8] {
    type Item<'a> = Vec<Buffer>;

    fn slice<'a>(
        component: ComponentIdentifier,
        array: &'a dyn ArrowArray,
        component_spans: impl Iterator<Item = Span<usize>> + 'a,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        slice_as_u8(component, array, component_spans)
    }
}

impl_buffer_native_type!(arrow::array::types::UInt16Type, u16);
impl_buffer_native_type!(arrow::array::types::UInt32Type, u32);
impl_buffer_native_type!(arrow::array::types::UInt64Type, u64);
// impl_buffer_native_type!(arrow::array::types::UInt128Type, u128);
impl_buffer_native_type!(arrow::array::types::Int8Type, i8);
impl_buffer_native_type!(arrow::array::types::Int16Type, i16);
impl_buffer_native_type!(arrow::array::types::Int32Type, i32);
impl_buffer_native_type!(arrow::array::types::Int64Type, i64);
// impl_buffer_native_type!(arrow::array::types::Int128Type, i128);
impl_buffer_native_type!(arrow::array::types::Float16Type, half::f16);
impl_buffer_native_type!(arrow::array::types::Float32Type, f32);
impl_buffer_native_type!(arrow::array::types::Float64Type, f64);

/// The actual implementation of `impl_array_list_native_type!`, so that we don't have to work in a macro.
fn slice_as_array_list_native<'a, const N: usize, P, T>(
    component: ComponentIdentifier,
    array: &'a dyn ArrowArray,
    component_spans: impl Iterator<Item = Span<usize>> + 'a,
) -> impl Iterator<Item = Vec<&'a [[T; N]]>> + 'a
where
    [T; N]: bytemuck::Pod,
    P: ArrowPrimitiveType<Native = T>,
    T: ArrowNativeType + bytemuck::Pod,
{
    let Some(inner_list_array) = array.downcast_array_ref::<ArrowListArray>() else {
        error_on_downcast_failure(component, "ArrowListArray", array.data_type());
        return Either::Left(std::iter::empty());
    };

    let inner_offsets = inner_list_array.offsets();
    let inner_lengths = inner_offsets.lengths().collect_vec();

    let Some(fixed_size_list_array) = inner_list_array
        .values()
        .downcast_array_ref::<ArrowFixedSizeListArray>()
    else {
        error_on_downcast_failure(
            component,
            "ArrowFixedSizeListArray",
            inner_list_array.data_type(),
        );
        return Either::Left(std::iter::empty());
    };

    let Some(values) = fixed_size_list_array
        .values()
        .downcast_array_ref::<ArrowPrimitiveArray<P>>()
    else {
        error_on_downcast_failure(
            component,
            "ArrowPrimitiveArray<P>",
            fixed_size_list_array.data_type(),
        );
        return Either::Left(std::iter::empty());
    };

    let size = fixed_size_list_array.value_length() as usize;
    let values = values.values();

    // NOTE: No need for validity checks here, `iter_offsets` already takes care of that.
    Either::Right(component_spans.map(move |span| {
        let inner_offsets = &inner_offsets[span.range()];
        let inner_lengths = &inner_lengths[span.range()];
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
    ($primitive_type:ty, $native_type:ty) => {
        impl<const N: usize> ChunkComponentSlicer for &[[$native_type; N]]
        where
            [$native_type; N]: bytemuck::Pod,
        {
            type Item<'a> = Vec<&'a [[$native_type; N]]>;

            fn slice<'a>(
                component: ComponentIdentifier,
                array: &'a dyn ArrowArray,
                component_spans: impl Iterator<Item = Span<usize>> + 'a,
            ) -> impl Iterator<Item = Self::Item<'a>> {
                slice_as_array_list_native::<N, $primitive_type, $native_type>(
                    component,
                    array,
                    component_spans,
                )
            }
        }
    };
}

impl_array_list_native_type!(arrow::array::types::UInt8Type, u8);
impl_array_list_native_type!(arrow::array::types::UInt16Type, u16);
impl_array_list_native_type!(arrow::array::types::UInt32Type, u32);
impl_array_list_native_type!(arrow::array::types::UInt64Type, u64);
// impl_array_list_native_type!(arrow::array::types::UInt128Type, u128);
impl_array_list_native_type!(arrow::array::types::Int8Type, i8);
impl_array_list_native_type!(arrow::array::types::Int16Type, i16);
impl_array_list_native_type!(arrow::array::types::Int32Type, i32);
impl_array_list_native_type!(arrow::array::types::Int64Type, i64);
// impl_array_list_native_type!(arrow::array::types::Int128Type, i128);
impl_array_list_native_type!(arrow::array::types::Float16Type, half::f16);
impl_array_list_native_type!(arrow::array::types::Float32Type, f32);
impl_array_list_native_type!(arrow::array::types::Float64Type, f64);

impl ChunkComponentSlicer for String {
    type Item<'a> = Vec<ArrowString>;

    fn slice<'a>(
        component: ComponentIdentifier,
        array: &'a dyn ArrowArray,
        component_spans: impl Iterator<Item = Span<usize>> + 'a,
    ) -> impl Iterator<Item = Vec<ArrowString>> {
        let Some(utf8_array) = array.downcast_array_ref::<ArrowStringArray>() else {
            error_on_downcast_failure(component, "ArrowStringArray", array.data_type());
            return Either::Left(std::iter::empty());
        };

        let values = utf8_array.values().clone();
        let offsets = utf8_array.offsets().clone();
        let lengths = offsets.lengths().collect_vec();

        // NOTE: No need for validity checks here, `component_spans` already takes care of that.
        Either::Right(component_spans.map(move |range| {
            let offsets = &offsets[range.range()];
            let lengths = &lengths[range.range()];
            izip!(offsets, lengths)
                .map(|(&idx, &len)| ArrowString::from(values.slice_with_length(idx as _, len)))
                .collect_vec()
        }))
    }
}

/// Like `slice::<String>()` but distinguishes between null and empty strings:
/// `None` for null entries, `Some("")` for an explicitly-empty string.
///
/// Prefer this over `slice::<String>()` when null carries meaning (e.g. a state reset):
/// the Arrow spec allows null slots to span arbitrary garbage bytes, so a plain values-buffer
/// slice is not guaranteed to yield an empty string for them.
///
/// NOTE: If null and `""` are treated the same by every caller, this slicer is removable in
/// practice: known writers (arrow-rs, pyarrow, C++) emit zero-length null slots, so
/// `slice::<String>()` yields `""` for them. But chunks come from the wire, so a writer that
/// exploits the spec's leeway would silently turn resets into phantom garbage values.
impl ChunkComponentSlicer for Option<String> {
    type Item<'a> = StringOptSliceIter<'a>;

    fn slice<'a>(
        component: ComponentIdentifier,
        array: &'a dyn ArrowArray,
        component_spans: impl Iterator<Item = Span<usize>> + 'a,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        if let Some(utf8_array) = array.downcast_array_ref::<ArrowStringArray>() {
            Either::Right(Either::Left(
                slice_as_opt_string(utf8_array, component_spans)
                    .map(|batch| StringOptSliceIter(Either::Left(batch))),
            ))
        } else if let Some(large_utf8_array) = array.downcast_array_ref::<ArrowLargeStringArray>() {
            Either::Right(Either::Right(
                slice_as_opt_string(large_utf8_array, component_spans)
                    .map(|batch| StringOptSliceIter(Either::Right(batch))),
            ))
        } else {
            error_on_downcast_failure(
                component,
                "ArrowStringArray or ArrowLargeStringArray",
                array.data_type(),
            );
            Either::Left(std::iter::empty())
        }
    }
}

/// The shared implementation of `slice::<Option<String>>()` for `Utf8` and `LargeUtf8` arrays.
fn slice_as_opt_string<'a, O: OffsetSizeTrait>(
    string_array: &'a ArrowGenericStringArray<O>,
    component_spans: impl Iterator<Item = Span<usize>> + 'a,
) -> impl Iterator<Item = GenericStringOptSliceIter<'a, O>> + 'a {
    let values = string_array.values();
    let offsets: &[O] = string_array.offsets();
    let nulls = string_array.nulls();

    component_spans.map(move |span| GenericStringOptSliceIter {
        values,
        offsets,
        nulls,
        range: span.range(),
    })
}

/// The offset-width-generic implementation behind [`StringOptSliceIter`].
struct GenericStringOptSliceIter<'a, O: OffsetSizeTrait> {
    values: &'a Buffer,

    /// The array's offsets, `len + 1` entries: element `i` spans `offsets[i]..offsets[i + 1]`.
    offsets: &'a [O],
    nulls: Option<&'a ArrowNullBuffer>,
    range: std::ops::Range<usize>,
}

impl<O: OffsetSizeTrait> Iterator for GenericStringOptSliceIter<'_, O> {
    type Item = Option<ArrowString>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let i = self.range.next()?;
        Some(if self.nulls.is_some_and(|nulls| !nulls.is_valid(i)) {
            None
        } else {
            let start = self.offsets[i].as_usize();
            let end = self.offsets[i + 1].as_usize();
            Some(ArrowString::from(
                self.values.slice_with_length(start, end - start),
            ))
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}

impl<O: OffsetSizeTrait> ExactSizeIterator for GenericStringOptSliceIter<'_, O> {}

/// Lazily yields `Option<ArrowString>` for one component batch (span) of a string column:
/// `None` for null slots, `Some(string)` otherwise (including `Some("")` for explicitly-empty
/// strings).
///
/// Yielded by `slice::<Option<String>>()`. Wraps both offset widths (`Utf8` and `LargeUtf8`).
pub struct StringOptSliceIter<'a>(
    Either<GenericStringOptSliceIter<'a, i32>, GenericStringOptSliceIter<'a, i64>>,
);

impl Iterator for StringOptSliceIter<'_> {
    type Item = Option<ArrowString>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl ExactSizeIterator for StringOptSliceIter<'_> {}

impl ChunkComponentSlicer for bool {
    type Item<'a> = ArrowBooleanBuffer;

    fn slice<'a>(
        component: ComponentIdentifier,
        array: &'a dyn ArrowArray,
        component_spans: impl Iterator<Item = Span<usize>> + 'a,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        let Some(values) = array.downcast_array_ref::<ArrowBooleanArray>() else {
            error_on_downcast_failure(component, "ArrowBooleanArray", array.data_type());
            return Either::Left(std::iter::empty());
        };
        let values = values.values().clone();

        // NOTE: No need for validity checks here, `component_spans` already takes care of that.
        Either::Right(
            component_spans.map(move |Span { start, len }| values.clone().slice(start, len)),
        )
    }
}

/// Lazily yields `Option<bool>` for one component batch (span) of a boolean column:
/// `None` for null slots, `Some(value)` otherwise.
///
/// Yielded by `slice::<Option<bool>>()`, which is like `slice::<bool>()` but distinguishes
/// null entries.
///
/// Note: for booleans, we can't use [`NativeOptSliceIter`] because it reads values through a
/// `&[T]` slice view, but an Arrow `BooleanArray` is bit-packed (8 per byte).
pub struct BoolOptSliceIter<'a> {
    values: &'a ArrowBooleanBuffer,
    nulls: Option<&'a ArrowNullBuffer>,
    range: std::ops::Range<usize>,
}

impl Iterator for BoolOptSliceIter<'_> {
    type Item = Option<bool>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let i = self.range.next()?;
        Some(if self.nulls.is_some_and(|nulls| !nulls.is_valid(i)) {
            None
        } else {
            Some(self.values.value(i))
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}

impl ExactSizeIterator for BoolOptSliceIter<'_> {}

impl ChunkComponentSlicer for Option<bool> {
    type Item<'a> = BoolOptSliceIter<'a>;

    fn slice<'a>(
        component: ComponentIdentifier,
        array: &'a dyn ArrowArray,
        component_spans: impl Iterator<Item = Span<usize>> + 'a,
    ) -> impl Iterator<Item = Self::Item<'a>> {
        let Some(boolean_array) = array.downcast_array_ref::<ArrowBooleanArray>() else {
            error_on_downcast_failure(component, "ArrowBooleanArray", array.data_type());
            return Either::Left(std::iter::empty());
        };
        let values = boolean_array.values();
        let nulls = boolean_array.nulls();

        Either::Right(component_spans.map(move |span| BoolOptSliceIter {
            values,
            nulls,
            range: span.range(),
        }))
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

        let row_id = *self.chunk.row_ids_slice().get(i)?;

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
        timeline: &TimelineName,
    ) -> impl Iterator<Item = (TimeInt, RowId)> + use<> {
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
    span: Span<usize>,
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
            span: Span::default(),
        }
    }
}

impl<C> ChunkComponentIterItem<C> {
    #[inline]
    pub fn as_slice(&self) -> &[C] {
        &self.values[self.span.range()]
    }
}

impl<C> std::ops::Deref for ChunkComponentIterItem<C> {
    type Target = [C];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<C: Component, IO: Iterator<Item = Span<usize>>> Iterator for ChunkComponentIter<C, IO> {
    type Item = ChunkComponentIterItem<C>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.offsets.next().map(move |span| ChunkComponentIterItem {
            values: Arc::clone(&self.values),
            span,
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
        component: ComponentIdentifier,
    ) -> ChunkComponentIter<C, impl Iterator<Item = Span<usize>> + '_ + use<'_, C>> {
        let Some(list_array) = self.components.get_array(component) else {
            return ChunkComponentIter {
                values: Arc::new(vec![]),
                offsets: Either::Left(std::iter::empty()),
            };
        };

        let values = arrow::array::ArrayRef::from(list_array.values().clone());
        let values = match C::from_arrow(&values) {
            Ok(values) => values,
            Err(err) => {
                re_log::debug_panic!(
                    "deserialization failed for {}, data discarded: {}",
                    C::name(),
                    re_error::format_ref(&err),
                );

                re_log::error_once!(
                    "deserialization failed for {}, data discarded: {}",
                    C::name(),
                    re_error::format_ref(&err),
                );

                return ChunkComponentIter {
                    values: Arc::new(vec![]),
                    offsets: Either::Left(std::iter::empty()),
                };
            }
        };

        // NOTE: No need for validity checks here, `iter_offsets` already takes care of that.
        ChunkComponentIter {
            values: Arc::new(values),
            offsets: Either::Right(self.iter_component_offsets(component)),
        }
    }
}

// ---

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arrow::array::{BooleanArray, Float64Array, LargeStringArray, StringArray};
    use itertools::{Itertools as _, izip};
    use re_log_types::example_components::{MyPoint, MyPoints};
    use re_log_types::{EntityPath, TimeInt, TimePoint};
    use re_span::Span;
    use re_types_core::{ArrowString, ComponentIdentifier};

    use super::ChunkComponentSlicer;
    use crate::{Chunk, RowId, Timeline};

    /// Builds a chunk with one `MyPoints::points` row per `(timepoint, has_component)` entry.
    ///
    /// Rows with `has_component == false` leave the component null, making the array nullable.
    fn timepoint_chunk(rows: impl IntoIterator<Item = (TimePoint, bool)>) -> Chunk {
        let mut builder = Chunk::builder("this/that");
        for (i, (timepoint, has_component)) in rows.into_iter().enumerate() {
            let points = [MyPoint::new(i as f32, i as f32)];
            builder = builder.with_sparse_component_batches(
                RowId::new(),
                timepoint,
                [(
                    MyPoints::descriptor_points(),
                    has_component.then_some(&points as _),
                )],
            );
        }
        builder.build().expect("valid chunk")
    }

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
                .with_component_batches(
                    row_id1,
                    timepoint1,
                    [(MyPoints::descriptor_points(), points1 as _)],
                )
                .with_component_batches(
                    row_id2,
                    timepoint2,
                    [(MyPoints::descriptor_points(), points2 as _)],
                )
                .with_component_batches(
                    row_id3,
                    timepoint3,
                    [(MyPoints::descriptor_points(), points3 as _)],
                )
                .with_component_batches(
                    row_id4,
                    timepoint4,
                    [(MyPoints::descriptor_points(), points4 as _)],
                )
                .with_component_batches(
                    row_id5,
                    timepoint5,
                    [(MyPoints::descriptor_points(), points5 as _)],
                )
                .build()?,
        );

        {
            let got = Arc::clone(&chunk)
                .iter_indices_owned(timeline_frame.name())
                .collect_vec();
            let expected = izip!(
                chunk
                    .timelines
                    .get(timeline_frame.name())
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
    fn iter_component_timepoints_temporal() {
        let timeline_frame = Timeline::new_sequence("frame");
        let timeline_other = Timeline::new_sequence("other");

        let timepoint1 = TimePoint::from([(timeline_frame, 10), (timeline_other, 1)]);
        let timepoint2 = TimePoint::from([(timeline_frame, 20), (timeline_other, 2)]);
        let timepoint3 = TimePoint::from([(timeline_frame, 30), (timeline_other, 3)]);

        let chunk = timepoint_chunk([
            (timepoint1.clone(), true),
            (timepoint2.clone(), true),
            (timepoint3.clone(), true),
        ]);
        let expected = vec![timepoint1, timepoint2, timepoint3];
        similar_asserts::assert_eq!(
            expected,
            chunk
                .iter_component_timepoints(MyPoints::descriptor_points().component)
                .collect_vec()
        );
    }

    #[test]
    fn iter_component_timepoints_temporal_sparse() {
        let timeline_frame = Timeline::new_sequence("frame");
        let timeline_other = Timeline::new_sequence("other");

        let timepoint1 = TimePoint::from([(timeline_frame, 10), (timeline_other, 1)]);
        let timepoint2 = TimePoint::from([(timeline_frame, 20), (timeline_other, 2)]);
        let timepoint3 = TimePoint::from([(timeline_frame, 30), (timeline_other, 3)]);

        let chunk = timepoint_chunk([
            (timepoint1.clone(), true),
            (timepoint2, false),
            (timepoint3.clone(), true),
        ]);
        let expected = vec![timepoint1, timepoint3];
        similar_asserts::assert_eq!(
            expected,
            chunk
                .iter_component_timepoints(MyPoints::descriptor_points().component)
                .collect_vec()
        );
    }

    #[test]
    fn iter_component_timepoints_static() {
        let chunk = timepoint_chunk((0..3).map(|_| (TimePoint::default(), true)));
        assert!(chunk.is_static());
        let expected = vec![TimePoint::default(); 3];
        similar_asserts::assert_eq!(
            expected,
            chunk
                .iter_component_timepoints(MyPoints::descriptor_points().component)
                .collect_vec()
        );
    }

    #[test]
    fn iter_component_timepoints_static_sparse() {
        let chunk = timepoint_chunk([
            (TimePoint::default(), true),
            (TimePoint::default(), false),
            (TimePoint::default(), true),
        ]);
        assert!(chunk.is_static());
        let expected = vec![TimePoint::default(); 2];
        similar_asserts::assert_eq!(
            expected,
            chunk
                .iter_component_timepoints(MyPoints::descriptor_points().component)
                .collect_vec()
        );
    }

    #[test]
    fn iter_component_timepoints_missing_component() {
        let timepoint = TimePoint::from([
            (Timeline::new_sequence("frame"), 10),
            (Timeline::new_sequence("other"), 1),
        ]);
        let chunk = timepoint_chunk([(timepoint, true)]);
        let got = chunk
            .iter_component_timepoints("non_existing_component".into())
            .collect_vec();
        assert!(got.is_empty());
    }

    #[test]
    fn iter_timepoints_temporal() {
        let timeline_frame = Timeline::new_sequence("frame");
        let timeline_other = Timeline::new_sequence("other");

        let timepoint1 = TimePoint::from([(timeline_frame, 10), (timeline_other, 1)]);
        let timepoint2 = TimePoint::from([(timeline_frame, 20), (timeline_other, 2)]);

        let chunk = timepoint_chunk([(timepoint1.clone(), true), (timepoint2.clone(), true)]);
        let expected = vec![timepoint1, timepoint2];
        similar_asserts::assert_eq!(expected, chunk.iter_timepoints().collect_vec());
    }

    #[test]
    fn iter_timepoints_static() {
        let chunk = timepoint_chunk((0..3).map(|_| (TimePoint::default(), true)));
        assert!(chunk.is_static());
        let expected = vec![TimePoint::default(); 3];
        similar_asserts::assert_eq!(expected, chunk.iter_timepoints().collect_vec());
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
                .with_component_batches(
                    row_id1,
                    TimePoint::default(),
                    [(MyPoints::descriptor_points(), points1 as _)],
                )
                .with_component_batches(
                    row_id2,
                    TimePoint::default(),
                    [(MyPoints::descriptor_points(), points2 as _)],
                )
                .with_component_batches(
                    row_id3,
                    TimePoint::default(),
                    [(MyPoints::descriptor_points(), points3 as _)],
                )
                .with_component_batches(
                    row_id4,
                    TimePoint::default(),
                    [(MyPoints::descriptor_points(), points4 as _)],
                )
                .with_component_batches(
                    row_id5,
                    TimePoint::default(),
                    [(MyPoints::descriptor_points(), points5 as _)],
                )
                .build()?,
        );

        {
            let got = Arc::clone(&chunk)
                .iter_indices_owned(timeline_frame.name())
                .collect_vec();
            let expected = izip!(std::iter::repeat(TimeInt::STATIC), chunk.row_ids()).collect_vec();

            similar_asserts::assert_eq!(expected, got);
        }

        Ok(())
    }

    // The `Option<T>` slicer tests: element-level validity must be preserved —
    // including across multi-element spans and on sliced arrays with a nonzero offset.

    /// Materializes each per-span batch into a `Vec` so tests can assert on plain values,
    /// regardless of whether the slicer yields lazy iterators or collected containers.
    fn slice_all<'a, S: ChunkComponentSlicer>(
        array: &'a dyn arrow::array::Array,
        spans: impl IntoIterator<Item = Span<usize>> + 'a,
    ) -> Vec<Vec<<S::Item<'a> as IntoIterator>::Item>>
    where
        S::Item<'a>: IntoIterator,
    {
        S::slice(ComponentIdentifier::from("test"), array, spans.into_iter())
            .map(|batch| batch.into_iter().collect())
            .collect()
    }

    #[test]
    fn option_f64() {
        let array = Float64Array::from(vec![Some(1.0), None, Some(3.0), Some(4.0), None]);
        let spans = [Span { start: 0, len: 2 }, Span { start: 2, len: 3 }];
        assert_eq!(
            slice_all::<Option<f64>>(&array, spans),
            vec![vec![Some(1.0), None], vec![Some(3.0), Some(4.0), None],]
        );

        // Nonzero-offset slice: logical elements [None, 3.0, 4.0].
        let sliced = array.slice(1, 3);
        assert_eq!(
            slice_all::<Option<f64>>(&sliced, [Span { start: 0, len: 3 }]),
            vec![vec![None, Some(3.0), Some(4.0)]]
        );
    }

    #[test]
    fn option_bool() {
        let array = BooleanArray::from(vec![Some(true), None, Some(false), None]);
        let spans = [Span { start: 0, len: 2 }, Span { start: 2, len: 2 }];
        assert_eq!(
            slice_all::<Option<bool>>(&array, spans),
            vec![vec![Some(true), None], vec![Some(false), None]]
        );

        let sliced = array.slice(1, 3);
        assert_eq!(
            slice_all::<Option<bool>>(&sliced, [Span { start: 0, len: 3 }]),
            vec![vec![None, Some(false), None]]
        );
    }

    #[test]
    fn option_string_distinguishes_null_and_empty() {
        let array = StringArray::from(vec![Some("a"), None, Some(""), Some("d")]);
        let spans = [Span { start: 0, len: 2 }, Span { start: 2, len: 2 }];
        assert_eq!(
            slice_all::<Option<String>>(&array, spans),
            vec![
                vec![Some(ArrowString::from("a")), None],
                vec![Some(ArrowString::from("")), Some(ArrowString::from("d"))],
            ]
        );

        // Nonzero-offset slice: logical elements [None, "", "d"].
        let sliced = array.slice(1, 3);
        assert_eq!(
            slice_all::<Option<String>>(&sliced, [Span { start: 0, len: 3 }]),
            vec![vec![
                None,
                Some(ArrowString::from("")),
                Some(ArrowString::from("d"))
            ]]
        );
    }

    #[test]
    fn option_large_string() {
        let array = LargeStringArray::from(vec![Some("a"), None, Some("c")]);
        assert_eq!(
            slice_all::<Option<String>>(&array, [Span { start: 0, len: 3 }]),
            vec![vec![
                Some(ArrowString::from("a")),
                None,
                Some(ArrowString::from("c"))
            ]]
        );
    }
}
