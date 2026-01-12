use std::iter::repeat_n;
use std::sync::Arc;

use arrow::array::{
    Array, ArrayData, ArrayRef, ArrowPrimitiveType, BooleanArray, FixedSizeListArray, ListArray,
    PrimitiveArray, UInt32Array, new_empty_array,
};
use arrow::buffer::{NullBuffer, OffsetBuffer};
use arrow::datatypes::{DataType, Field};
use arrow::error::ArrowError;
use itertools::Itertools as _;

// ---------------------------------------------------------------------------------

/// Downcast an arrow array to another array, without having to go via `Any`.
pub trait ArrowArrayDowncastRef<'a>: 'a {
    /// Downcast an arrow array to another array, without having to go via `Any`.
    fn downcast_array_ref<T: Array + 'static>(self) -> Option<&'a T>;

    /// Similar to `downcast_array_ref`, but returns an error in case the downcast
    /// returns `None`.
    fn try_downcast_array_ref<T: Array + 'static>(self) -> Result<&'a T, ArrowError>;

    /// Similar to `downcast_array_ref`, but returns an error in case the downcast
    /// returns `None`.
    fn try_downcast_array<T: Array + Clone + 'static>(self) -> Result<T, ArrowError>;
}

impl<'a> ArrowArrayDowncastRef<'a> for &'a dyn Array {
    fn downcast_array_ref<T: Array + 'static>(self) -> Option<&'a T> {
        self.as_any().downcast_ref()
    }

    fn try_downcast_array_ref<T: Array + 'static>(self) -> Result<&'a T, ArrowError> {
        self.downcast_array_ref::<T>().ok_or_else(|| {
            ArrowError::CastError(format!(
                "Failed to downcast array of type {} to {}",
                self.data_type(),
                std::any::type_name::<T>(),
            ))
        })
    }

    /// Similar to `downcast_array_ref`, but returns an error in case the downcast
    /// returns `None`.
    fn try_downcast_array<T: Array + Clone + 'static>(self) -> Result<T, ArrowError> {
        Ok(self.try_downcast_array_ref::<T>()?.clone())
    }
}

impl<'a> ArrowArrayDowncastRef<'a> for &'a ArrayRef {
    fn downcast_array_ref<T: Array + 'static>(self) -> Option<&'a T> {
        self.as_any().downcast_ref()
    }

    fn try_downcast_array_ref<T: Array + 'static>(self) -> Result<&'a T, ArrowError> {
        self.downcast_array_ref::<T>().ok_or_else(|| {
            ArrowError::CastError(format!(
                "Failed to downcast array of type {} to {}",
                self.data_type(),
                std::any::type_name::<T>(),
            ))
        })
    }

    /// Similar to `downcast_array_ref`, but returns an error in case the downcast
    /// returns `None`.
    fn try_downcast_array<T: Array + Clone + 'static>(self) -> Result<T, ArrowError> {
        Ok(self.try_downcast_array_ref::<T>()?.clone())
    }
}

// ---------------------------------------------------------------------------------

#[inline]
pub fn into_arrow_ref(array: impl Array + 'static) -> ArrayRef {
    std::sync::Arc::new(array)
}

/// Returns true if the given `list_array` is semantically empty.
///
/// Semantic emptiness is defined as either one of these:
/// * The list is physically empty (literally no data).
/// * The list only contains null entries, or empty arrays, or a mix of both.
#[inline]
pub fn is_list_array_semantically_empty(list_array: &ListArray) -> bool {
    list_array.values().is_empty()
}

/// Create a sparse list-array out of an array of arrays.
///
/// All arrays must have the same datatype.
///
/// Returns `None` if `arrays` is empty.
#[inline]
pub fn arrays_to_list_array_opt(arrays: &[Option<&dyn Array>]) -> Option<ListArray> {
    let datatype = arrays
        .iter()
        .flatten()
        .map(|array| array.data_type().clone())
        .next()?;
    arrays_to_list_array(datatype, arrays)
}

/// Create a sparse list-array out of an array of arrays.
///
/// Returns `None` if any of the specified `arrays` doesn't match the given `array_datatype`.
///
/// Returns an empty list if `arrays` is empty.
pub fn arrays_to_list_array(
    array_datatype: DataType,
    arrays: &[Option<&dyn Array>],
) -> Option<ListArray> {
    let arrays_dense = arrays.iter().flatten().copied().collect_vec();

    let data = if arrays_dense.is_empty() {
        new_empty_array(&array_datatype)
    } else {
        re_tracing::profile_scope!("concatenate", arrays_dense.len().to_string());
        concat_arrays(&arrays_dense)
            .map_err(|err| {
                re_log::warn_once!("failed to concatenate arrays: {err}");
                err
            })
            .ok()?
    };

    let nullable = true;
    let field = Field::new_list_field(array_datatype, nullable);

    let offsets = OffsetBuffer::from_lengths(
        arrays
            .iter()
            .map(|array| array.map_or(0, |array| array.len())),
    );

    #[expect(clippy::from_iter_instead_of_collect)]
    let nulls = NullBuffer::from_iter(arrays.iter().map(Option::is_some));

    Some(ListArray::new(field.into(), offsets, data, nulls.into()))
}

/// Given a sparse [`ListArray`] (i.e. an array with a nulls bitmap that contains at least
/// one falsy value), returns a dense [`ListArray`] that only contains the non-null values from
/// the original list.
///
/// This is a no-op if the original array is already dense.
pub fn sparse_list_array_to_dense_list_array(list_array: &ListArray) -> ListArray {
    if list_array.is_empty() {
        return list_array.clone();
    }

    let is_empty = list_array.nulls().is_some_and(|nulls| nulls.is_empty());
    if is_empty {
        return list_array.clone();
    }

    let offsets = OffsetBuffer::from_lengths(list_array.iter().flatten().map(|array| array.len()));

    let fields = list_array_fields(list_array);

    ListArray::new(fields, offsets, list_array.values().clone(), None)
}

fn list_array_fields(list_array: &arrow::array::GenericListArray<i32>) -> std::sync::Arc<Field> {
    match list_array.data_type() {
        DataType::List(fields) | DataType::LargeList(fields) => fields,
        _ => unreachable!("The GenericListArray constructor guaranteed we can't get here"),
    }
    .clone()
}

/// Create a new [`ListArray`] of target length by appending null values to its back.
///
/// This will share the same child data array buffer, but will create new offset and nulls buffers.
pub fn pad_list_array_back(list_array: &ListArray, target_len: usize) -> ListArray {
    let missing_len = target_len.saturating_sub(list_array.len());
    if missing_len == 0 {
        return list_array.clone();
    }

    let fields = list_array_fields(list_array);

    let offsets = {
        OffsetBuffer::from_lengths(
            list_array
                .iter()
                .map(|array| array.map_or(0, |array| array.len()))
                .chain(repeat_n(0, missing_len)),
        )
    };

    let values = list_array.values().clone();

    let nulls = {
        if let Some(nulls) = list_array.nulls() {
            #[expect(clippy::from_iter_instead_of_collect)]
            NullBuffer::from_iter(nulls.iter().chain(repeat_n(false, missing_len)))
        } else {
            #[expect(clippy::from_iter_instead_of_collect)]
            NullBuffer::from_iter(
                repeat_n(true, list_array.len()).chain(repeat_n(false, missing_len)),
            )
        }
    };

    ListArray::new(fields, offsets, values, Some(nulls))
}

/// Create a new [`ListArray`] of target length by appending null values to its front.
///
/// This will share the same child data array buffer, but will create new offset and nulls buffers.
pub fn pad_list_array_front(list_array: &ListArray, target_len: usize) -> ListArray {
    let missing_len = target_len.saturating_sub(list_array.len());
    if missing_len == 0 {
        return list_array.clone();
    }

    let fields = list_array_fields(list_array);

    let offsets = {
        OffsetBuffer::from_lengths(
            repeat_n(0, missing_len).chain(
                list_array
                    .iter()
                    .map(|array| array.map_or(0, |array| array.len())),
            ),
        )
    };

    let values = list_array.values().clone();

    let nulls = {
        if let Some(nulls) = list_array.nulls() {
            #[expect(clippy::from_iter_instead_of_collect)]
            NullBuffer::from_iter(repeat_n(false, missing_len).chain(nulls.iter()))
        } else {
            #[expect(clippy::from_iter_instead_of_collect)]
            NullBuffer::from_iter(
                repeat_n(false, missing_len).chain(repeat_n(true, list_array.len())),
            )
        }
    };

    ListArray::new(fields, offsets, values, Some(nulls))
}

/// Returns a new [[`ListArray`]] with len `entries`.
///
/// Each entry will be an empty array of the given `child_datatype`.
pub fn new_list_array_of_empties(child_datatype: &DataType, len: usize) -> ListArray {
    let empty_array = new_empty_array(child_datatype);

    let offsets = OffsetBuffer::from_lengths(repeat_n(0, len));

    let nullable = true;
    ListArray::new(
        Field::new_list_field(empty_array.data_type().clone(), nullable).into(),
        offsets,
        empty_array,
        None,
    )
}

/// Applies a [`arrow::compute::concat`] kernel to the given `arrays`.
///
/// Early outs where it makes sense (e.g. `arrays.len() == 1`).
///
/// Returns an error if the arrays don't share the exact same datatype.
pub fn concat_arrays(arrays: &[&dyn Array]) -> arrow::error::Result<ArrayRef> {
    #[expect(clippy::disallowed_methods)] // that's the whole point
    let mut array = arrow::compute::concat(arrays)?;
    array.shrink_to_fit(); // VERY IMPORTANT! https://github.com/rerun-io/rerun/issues/7222
    Ok(array)
}

/// Applies a [filter] kernel to the given `array`.
///
/// Panics iff the length of the filter doesn't match the length of the array.
///
/// In release builds, filters are allowed to have null entries (they will be interpreted as `false`).
/// In debug builds, null entries will panic.
///
/// Note: a `filter` kernel _copies_ the data in order to make the resulting arrays contiguous in memory.
///
/// Takes care of up- and down-casting the data back and forth on behalf of the caller.
///
/// [filter]: arrow::compute::filter
pub fn filter_array<A: Array + Clone + 'static>(array: &A, filter: &BooleanArray) -> A {
    assert_eq!(
        array.len(),
        filter.len(),
        "the length of the filter must match the length of the array (the underlying kernel will panic otherwise)",
    );
    debug_assert!(
        filter.nulls().is_none(),
        "filter masks with nulls bits are technically valid, but generally a sign that something went wrong",
    );

    #[expect(clippy::disallowed_methods)] // that's the whole point
    #[expect(clippy::unwrap_used)]
    let mut array = arrow::compute::filter(array, filter)
        // Unwrap: this literally cannot fail.
        .unwrap()
        .as_any()
        .downcast_ref::<A>()
        // Unwrap: that's initial type that we got.
        .unwrap()
        .clone();
    array.shrink_to_fit(); // VERY IMPORTANT! https://github.com/rerun-io/rerun/issues/7222
    array
}

/// Applies a [take] kernel to the given `array`.
///
/// In release builds, indices are allowed to have null entries (they will be taken as `null`s).
/// In debug builds, null entries will panic.
///
/// Note: a `take` kernel _copies_ the data in order to make the resulting arrays contiguous in memory.
///
/// Takes care of up- and down-casting the data back and forth on behalf of the caller.
///
/// [take]: arrow::compute::take
//
// TODO(cmc): in an ideal world, a `take` kernel should merely _slice_ the data and avoid any allocations/copies
// where possible (e.g. list-arrays).
// That is not possible with vanilla [`ListArray`]s since they don't expose any way to encode optional lengths,
// in addition to offsets.
// For internal stuff, we could perhaps provide a custom implementation that returns a `DictionaryArray` instead?
pub fn take_array<A, O>(array: &A, indices: &PrimitiveArray<O>) -> A
where
    A: Array + Clone + 'static,
    O: ArrowPrimitiveType,
    O::Native: std::ops::Add<Output = O::Native>,
{
    use arrow::datatypes::ArrowNativeTypeOp as _;

    debug_assert!(
        indices.nulls().is_none(),
        "index arrays with nulls bits are technically valid, but generally a sign that something went wrong",
    );

    if indices.len() == array.len() {
        let indices = indices.values();

        let starts_at_zero = || indices[0] == O::Native::ZERO;
        let is_consecutive = || {
            indices
                .windows(2)
                .all(|values| values[1] == values[0] + O::Native::ONE)
        };

        if starts_at_zero() && is_consecutive() {
            #[expect(clippy::unwrap_used)]
            return array
                .clone()
                .as_any()
                .downcast_ref::<A>()
                // Unwrap: that's initial type that we got.
                .unwrap()
                .clone();
        }
    }

    #[expect(clippy::disallowed_methods)] // that's the whole point
    #[expect(clippy::unwrap_used)]
    let mut array = arrow::compute::take(array, indices, Default::default())
        // Unwrap: this literally cannot fail.
        .unwrap()
        .as_any()
        .downcast_ref::<A>()
        // Unwrap: that's initial type that we got.
        .unwrap()
        .clone();
    array.shrink_to_fit(); // VERY IMPORTANT! https://github.com/rerun-io/rerun/issues/7222
    array
}

/// Extract the element at `idx` from a `FixedSizeListArray`.
///
/// For example:
/// `[[1, 2], [3, 4], [5, 6]] -> [1, 3, 5]`
pub fn extract_fixed_size_array_element(
    data: &FixedSizeListArray,
    idx: u32,
) -> Result<ArrayRef, ArrowError> {
    let num_elements = data.value_length() as u32;
    let num_values = data.len() as u32;

    let indices = UInt32Array::from(
        (0..num_values)
            .map(|i| i * num_elements + idx)
            .collect::<Vec<_>>(),
    );

    // We have forbidden using arrow::take, but it really is what we want here
    // `take_array` results in an unwrap so it appears not to be the right choice.
    // TODO(jleibs): Follow up with cmc on if there's a different way to do this.
    #[expect(clippy::disallowed_methods)]
    arrow::compute::kernels::take::take(data.values(), &indices, None)
}

// ----------------------------------------------------------------------------

/// Convert `[A, B, null, D, …]` into `[[A], [B], null, [D], …]`.
pub fn wrap_in_list_array(field: &Field, array: ArrayRef) -> (Field, ListArray) {
    re_tracing::profile_function!();

    // The current code reuses the input array as the "item" array,
    // with an offset-buffer that is all one-length lists.
    // This means the function is zero-copy, which is good.
    // TODO(emilk): if the input is mostly nulls we should instead
    // reallocate the inner array so that it is dense (non-nullable),
    // and use an offset-buffer with zero-length lists for the nulls.

    debug_assert_eq!(field.data_type(), array.data_type());

    let item_field = Arc::new(Field::new(
        "item",
        field.data_type().clone(),
        field.is_nullable(),
    ));

    let offsets = OffsetBuffer::from_lengths(repeat_n(1, array.len()));
    let nulls = array.nulls().cloned();
    let list_array = ListArray::new(item_field, offsets, array, nulls);

    let list_field = Field::new(
        field.name().clone(),
        list_array.data_type().clone(),
        true, // All components in Rerun has "outer nullability"
    )
    .with_metadata(field.metadata().clone());

    (list_field, list_array)
}

#[test]
fn test_wrap_in_list_array() {
    use arrow::array::{Array as _, AsArray as _, Int32Array};
    use arrow::buffer::{NullBuffer, ScalarBuffer};
    use arrow::datatypes::{DataType, Int32Type};

    // Convert [42, 69, null, 1337] into [[42], [69], null, [1337]]
    let original_field = Field::new("item", DataType::Int32, true);
    let original_array = Int32Array::new(
        ScalarBuffer::from(vec![42, 69, -1, 1337]),
        Some(NullBuffer::from(vec![true, true, false, true])),
    );
    assert_eq!(original_array.len(), 4);
    assert_eq!(original_array.null_count(), 1);

    let (new_field, new_array) =
        wrap_in_list_array(&original_field, into_arrow_ref(original_array.clone()));

    assert_eq!(new_field.data_type(), new_array.data_type());
    assert_eq!(new_array.len(), original_array.len());
    assert_eq!(new_array.null_count(), original_array.null_count());
    assert_eq!(original_field.data_type(), &new_array.value_type());

    assert_eq!(
        new_array
            .value(0)
            .as_primitive::<Int32Type>()
            .values()
            .as_ref(),
        &[42]
    );
    assert_eq!(
        new_array
            .value(1)
            .as_primitive::<Int32Type>()
            .values()
            .as_ref(),
        &[69]
    );
    assert_eq!(
        new_array
            .value(3)
            .as_primitive::<Int32Type>()
            .values()
            .as_ref(),
        &[1337]
    );
}

// ---

/// Deep-slicing operation for Arrow arrays.
///
/// The data, offsets, bitmaps and any other buffers required will be reallocated, copied around, and patched
/// as much as required so that the resulting physical data becomes as packed as possible for the desired slice.
///
/// This is the erased version, see [`deep_slice_array`] for a typed implementation.
//
// TODO(cmc): optimize from there; future results should always match this baseline.
pub fn deep_slice_array_erased(
    array: &dyn arrow::array::Array,
    offset: usize,
    length: usize,
) -> ArrayRef {
    let data = array.to_data();

    let use_null_optimization = false;
    let mut data_sliced =
        arrow::array::MutableArrayData::new(vec![&data], use_null_optimization, length);

    data_sliced.extend(0, offset, offset + length);

    arrow::array::make_array(data_sliced.freeze())
}

/// Deep-slicing operation for Arrow arrays.
///
/// The data, offsets, bitmaps and any other buffers required will be reallocated, copied around, and patched
/// as much as required so that the resulting physical data becomes as packed as possible for the desired slice.
///
/// This is the erased version, see [`deep_slice_array_erased`] for a typed implementation.
//
// TODO(cmc): optimize from there; future results should always match this baseline.
pub fn deep_slice_array<T: Array + From<ArrayData>>(array: &T, offset: usize, length: usize) -> T {
    let data = array.to_data();

    let use_null_optimization = false;
    let mut data_sliced =
        arrow::array::MutableArrayData::new(vec![&data], use_null_optimization, length);

    data_sliced.extend(0, offset, offset + length);

    T::from(data_sliced.freeze())
}

#[cfg(test)]
#[expect(
    clippy::cast_possible_wrap,
    clippy::disallowed_methods,
    clippy::needless_pass_by_value
)]
mod tests {
    use arrow::array::{
        Array, ArrayRef, Float32Array, Int32Array, Int64Array, ListArray, RecordBatch, StructArray,
        UInt8Array, UnionArray,
    };
    use arrow::buffer::{OffsetBuffer, ScalarBuffer};
    use arrow::datatypes::{Field, UnionFields};
    use arrow::ipc::writer::StreamWriter;
    use std::sync::Arc;

    use super::*;

    #[test]
    fn deep_slice() {
        let mut output = String::new();

        let size = 100_000;
        let values: Vec<i32> = (0..size as i32).collect();

        let int_array = Int32Array::from(values);
        output += &print_info("int32", Arc::new(int_array.clone()), 25000, 50000);

        let offsets = OffsetBuffer::from_lengths(std::iter::repeat_n(1, size));
        let list_int_array = ListArray::try_new(
            Arc::new(arrow::datatypes::Field::new(
                "item",
                int_array.data_type().clone(),
                false,
            )),
            offsets,
            Arc::new(int_array.clone()),
            None,
        )
        .unwrap();
        output += &print_info(
            "list[int32]",
            Arc::new(list_int_array.clone()),
            25000,
            50000,
        );

        let offsets = OffsetBuffer::from_lengths(std::iter::repeat_n(1, size));
        let list_list_int_array = ListArray::try_new(
            Arc::new(arrow::datatypes::Field::new(
                "item",
                list_int_array.data_type().clone(),
                false,
            )),
            offsets,
            Arc::new(list_int_array.clone()),
            None,
        )
        .unwrap();
        output += &print_info(
            "list[list[int32]]",
            Arc::new(list_list_int_array.clone()),
            5000,
            5,
        );

        let struct_array = StructArray::new(
            vec![Field::new("i", int_array.data_type().clone(), false)].into(),
            vec![Arc::new(int_array.clone())],
            None,
        );
        output += &print_info(
            "struct{int32}",
            Arc::new(struct_array.clone()),
            25000,
            50000,
        );

        let offsets = OffsetBuffer::from_lengths(std::iter::repeat_n(1, size));
        let list_struct_int_array = ListArray::try_new(
            Arc::new(arrow::datatypes::Field::new(
                "item",
                struct_array.data_type().clone(),
                false,
            )),
            offsets,
            Arc::new(struct_array.clone()),
            None,
        )
        .unwrap();
        output += &print_info(
            "list[struct{int32}]",
            Arc::new(list_struct_int_array.clone()),
            25000,
            50000,
        );

        // union#dense{{u8,f32,i64}}
        {
            const NUM_TOTAL: usize = 100_000;

            let u8s = UInt8Array::from_iter_values(
                (0u32..).take(NUM_TOTAL).map(|i| (i % u8::MAX as u32) as u8),
            );
            let f32s =
                Float32Array::from_iter_values((0..).take(NUM_TOTAL / 3 + 1).map(|i| i as f32));
            let i64s = Int64Array::from_iter_values((0..).take(NUM_TOTAL / 3 + 1));

            let type_ids = vec![0, 1, 2];
            let fields = vec![
                Arc::new(Field::new("u8", u8s.data_type().clone(), true)),
                Arc::new(Field::new("f32", f32s.data_type().clone(), true)),
                Arc::new(Field::new("i64", i64s.data_type().clone(), true)),
            ];
            let union_fields = UnionFields::new(type_ids, fields);

            let type_id_buffer = ScalarBuffer::from(
                (0..NUM_TOTAL as i32)
                    .map(|i| (i % 3) as i8)
                    .collect::<Vec<_>>(),
            );
            let value_offsets =
                ScalarBuffer::from((0..NUM_TOTAL as i32).map(|i| i / 3).collect::<Vec<_>>());

            let children = vec![
                Arc::new(u8s) as ArrayRef,
                Arc::new(f32s) as ArrayRef,
                Arc::new(i64s) as ArrayRef,
            ];

            let array = Arc::new(
                UnionArray::try_new(union_fields, type_id_buffer, Some(value_offsets), children)
                    .unwrap(),
            ) as ArrayRef;

            let from = NUM_TOTAL / 4;
            let len = NUM_TOTAL / 2;
            output += &print_info("union#dense{{u8,f32,i64}}:", array, from, len);
        }

        // union#sparse{{u8,f32,i64}}
        {
            const NUM_TOTAL: usize = 100_000;

            let u8s = UInt8Array::from_iter_values(
                (0u32..).take(NUM_TOTAL).map(|i| (i % u8::MAX as u32) as u8),
            );
            let f32s = Float32Array::from_iter_values((0..).take(NUM_TOTAL).map(|i| i as f32));
            let i64s = Int64Array::from_iter_values((0..).take(NUM_TOTAL));

            let type_ids = vec![0, 1, 2];
            let fields = vec![
                Arc::new(Field::new("u8", u8s.data_type().clone(), true)),
                Arc::new(Field::new("f32", f32s.data_type().clone(), true)),
                Arc::new(Field::new("i64", i64s.data_type().clone(), true)),
            ];
            let union_fields = UnionFields::new(type_ids, fields);

            let type_id_buffer = ScalarBuffer::from(
                (0..NUM_TOTAL as i32)
                    .map(|i| (i % 3) as i8)
                    .collect::<Vec<_>>(),
            );

            let children = vec![
                Arc::new(u8s) as ArrayRef,
                Arc::new(f32s) as ArrayRef,
                Arc::new(i64s) as ArrayRef,
            ];

            let array = Arc::new(
                UnionArray::try_new(union_fields, type_id_buffer, None, children).unwrap(),
            ) as ArrayRef;

            let from = NUM_TOTAL / 4;
            let len = NUM_TOTAL / 2;
            output += &print_info("union#sparse{{u8,f32,i64}}:", array, from, len);
        }

        // union#dense{{list[u8],list[f32],list[i64]}}
        {
            const NUM_TOTAL: usize = 100_000;
            const NUM_PER_BATCH: usize = 5_000;

            let u8s = UInt8Array::from_iter_values(
                (0u32..)
                    .take(NUM_TOTAL / 2)
                    .map(|i| (i % u8::MAX as u32) as u8),
            );
            let f32s = Float32Array::from_iter_values((0..).take(NUM_TOTAL / 2).map(|i| i as f32));
            let i64s = Int64Array::from_iter_values((0..).take(NUM_TOTAL / 2));

            let offsets = OffsetBuffer::from_lengths(std::iter::repeat_n(
                NUM_PER_BATCH,
                (NUM_TOTAL / 3 + 1) / NUM_PER_BATCH + 1,
            ));
            let list_u8s = ListArray::try_new(
                Arc::new(arrow::datatypes::Field::new(
                    "item",
                    u8s.data_type().clone(),
                    false,
                )),
                offsets.clone(),
                Arc::new(u8s.clone()),
                None,
            )
            .unwrap();
            let list_f32s = ListArray::try_new(
                Arc::new(arrow::datatypes::Field::new(
                    "item",
                    f32s.data_type().clone(),
                    false,
                )),
                offsets.clone(),
                Arc::new(f32s.clone()),
                None,
            )
            .unwrap();
            let list_i64s = ListArray::try_new(
                Arc::new(arrow::datatypes::Field::new(
                    "item",
                    i64s.data_type().clone(),
                    false,
                )),
                offsets.clone(),
                Arc::new(i64s.clone()),
                None,
            )
            .unwrap();

            let type_ids = vec![0, 1, 2];
            let fields = vec![
                Arc::new(Field::new("u8_list", list_u8s.data_type().clone(), true)),
                Arc::new(Field::new("f32_list", list_f32s.data_type().clone(), true)),
                Arc::new(Field::new("i64_list", list_i64s.data_type().clone(), true)),
            ];
            let union_fields = UnionFields::new(type_ids, fields);

            let type_id_buffer = ScalarBuffer::from(
                (0..(NUM_TOTAL / NUM_PER_BATCH) as i32)
                    .map(|i| (i % 3) as i8)
                    .collect::<Vec<_>>(),
            );
            let value_offsets = ScalarBuffer::from(
                (0..(NUM_TOTAL / NUM_PER_BATCH) as i32)
                    .map(|i| i / 3)
                    .collect::<Vec<_>>(),
            );

            let children = vec![
                Arc::new(list_u8s) as ArrayRef,
                Arc::new(list_f32s) as ArrayRef,
                Arc::new(list_i64s) as ArrayRef,
            ];

            let array = Arc::new(
                UnionArray::try_new(union_fields, type_id_buffer, Some(value_offsets), children)
                    .unwrap(),
            ) as ArrayRef;

            let from = NUM_TOTAL / NUM_PER_BATCH / 4;
            let len = NUM_TOTAL / NUM_PER_BATCH / 2;
            output += &print_info(
                "union#dense{{list[u8],list[f32],list[i64]}}:",
                array,
                from,
                len,
            );
        }

        // union#sparse{{list[u8],list[f32],list[i64]}}
        {
            const NUM_TOTAL: usize = 100_000;
            const NUM_PER_BATCH: usize = 5_000;

            let u8s = UInt8Array::from_iter_values(
                (0u32..).take(NUM_TOTAL).map(|i| (i % u8::MAX as u32) as u8),
            );
            let f32s = Float32Array::from_iter_values((0..).take(NUM_TOTAL).map(|i| i as f32));
            let i64s = Int64Array::from_iter_values((0..).take(NUM_TOTAL));

            let offsets = OffsetBuffer::from_lengths(std::iter::repeat_n(
                NUM_PER_BATCH,
                NUM_TOTAL / NUM_PER_BATCH,
            ));
            let list_u8s = ListArray::try_new(
                Arc::new(arrow::datatypes::Field::new(
                    "item",
                    u8s.data_type().clone(),
                    false,
                )),
                offsets.clone(),
                Arc::new(u8s.clone()),
                None,
            )
            .unwrap();
            let list_f32s = ListArray::try_new(
                Arc::new(arrow::datatypes::Field::new(
                    "item",
                    f32s.data_type().clone(),
                    false,
                )),
                offsets.clone(),
                Arc::new(f32s.clone()),
                None,
            )
            .unwrap();
            let list_i64s = ListArray::try_new(
                Arc::new(arrow::datatypes::Field::new(
                    "item",
                    i64s.data_type().clone(),
                    false,
                )),
                offsets.clone(),
                Arc::new(i64s.clone()),
                None,
            )
            .unwrap();

            let type_ids = vec![0, 1, 2];
            let fields = vec![
                Arc::new(Field::new("u8_list", list_u8s.data_type().clone(), true)),
                Arc::new(Field::new("f32_list", list_f32s.data_type().clone(), true)),
                Arc::new(Field::new("i64_list", list_i64s.data_type().clone(), true)),
            ];
            let union_fields = UnionFields::new(type_ids, fields);

            let type_id_buffer = ScalarBuffer::from(
                (0..(NUM_TOTAL / NUM_PER_BATCH) as i32)
                    .map(|i| (i % 3) as i8)
                    .collect::<Vec<_>>(),
            );

            let children = vec![
                Arc::new(list_u8s) as ArrayRef,
                Arc::new(list_f32s) as ArrayRef,
                Arc::new(list_i64s) as ArrayRef,
            ];

            let array = Arc::new(
                UnionArray::try_new(union_fields, type_id_buffer, None, children).unwrap(),
            ) as ArrayRef;

            let from = NUM_TOTAL / NUM_PER_BATCH / 4;
            let len = NUM_TOTAL / NUM_PER_BATCH / 2;
            output += &print_info(
                "union#sparse{{list[u8],list[f32],list[i64]}}:",
                array,
                from,
                len,
            );
        }

        insta::assert_snapshot!("deep_slice_comparisons", output);
    }

    fn dump_array_stats(array: &dyn Array) -> String {
        let data = array.to_data();
        format!(
            "len={:10} array_size={:10} buf_size={:10} data.len={:10} data.array_size={:10} data.buf_size={:10} data.slice_size={:10}",
            array.len(),
            array.get_array_memory_size(),
            array.get_buffer_memory_size(),
            data.len(),
            data.get_array_memory_size(),
            data.get_buffer_memory_size(),
            data.get_slice_memory_size().unwrap(),
        )
    }

    fn dump_array_to_ipc(array: Arc<dyn Array>) -> usize {
        let schema = Arc::new(arrow::datatypes::Schema::new(vec![
            arrow::datatypes::Field::new("col", array.data_type().clone(), false),
        ]));

        let batch = RecordBatch::try_new(schema.clone(), vec![array]).unwrap();

        let mut buffer = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut buffer, &schema).unwrap();
            writer.write(&batch).unwrap();
            writer.finish().unwrap();
        }

        buffer.len()
    }

    fn print_info(descr: &str, array: Arc<dyn Array>, offset: usize, len: usize) -> String {
        let mut output = String::new();

        let from = offset;
        let to = offset + len;

        let sliced = array.slice(offset, len);
        let deep_sliced = deep_slice_array_erased(&array, offset, len);
        assert_eq!(&deep_sliced, &sliced);

        output += &format!("{descr}:\n");
        output += &format!(
            "array[0..]:          {} / IPC={:6}\n",
            dump_array_stats(&array),
            dump_array_to_ipc(array.clone()),
        );
        output += &format!(
            "slice[{from:5}..{to:5}]: {} / IPC={:6}\n",
            dump_array_stats(&sliced),
            dump_array_to_ipc(sliced.clone())
        );
        output += &format!(
            " deep[{from:5}..{to:5}]: {} / IPC={:6}\n",
            dump_array_stats(&deep_sliced),
            dump_array_to_ipc(deep_sliced.clone())
        );
        output += "\n";

        output
    }
}
