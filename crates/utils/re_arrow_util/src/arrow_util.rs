use arrow::{
    array::{Array, ArrayRef, ArrowPrimitiveType, BooleanArray, ListArray, PrimitiveArray},
    buffer::{NullBuffer, OffsetBuffer},
    datatypes::{DataType, Field},
};
use itertools::Itertools as _;

// ---

/// Downcast an arrow array to another array, without having to go via `Any`.
///
/// This is shorter, but also better: it means we don't accidentally downcast
/// an arrow2 array to an arrow1 array, or vice versa.
pub trait ArrowArrayDowncastRef {
    /// Downcast an arrow array to another array, without having to go via `Any`.
    ///
    /// This is shorter, but also better: it means we don't accidentally downcast
    /// an arrow2 array to an arrow1 array, or vice versa.
    fn downcast_array_ref<T: Array + 'static>(&self) -> Option<&T>;
}

impl ArrowArrayDowncastRef for dyn Array {
    fn downcast_array_ref<T: Array + 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref()
    }
}

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

/// An empty array of the given datatype.
pub fn new_empty_array(datatype: &DataType) -> ArrayRef {
    let capacity = 0;
    arrow::array::make_builder(datatype, capacity).finish()
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

    #[allow(clippy::from_iter_instead_of_collect)]
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
                .chain(std::iter::repeat(0).take(missing_len)),
        )
    };

    let values = list_array.values().clone();

    let nulls = {
        if let Some(nulls) = list_array.nulls() {
            #[allow(clippy::from_iter_instead_of_collect)]
            NullBuffer::from_iter(
                nulls
                    .iter()
                    .chain(std::iter::repeat(false).take(missing_len)),
            )
        } else {
            #[allow(clippy::from_iter_instead_of_collect)]
            NullBuffer::from_iter(
                std::iter::repeat(true)
                    .take(list_array.len())
                    .chain(std::iter::repeat(false).take(missing_len)),
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
            std::iter::repeat(0).take(missing_len).chain(
                list_array
                    .iter()
                    .map(|array| array.map_or(0, |array| array.len())),
            ),
        )
    };

    let values = list_array.values().clone();

    let nulls = {
        if let Some(nulls) = list_array.nulls() {
            #[allow(clippy::from_iter_instead_of_collect)]
            NullBuffer::from_iter(
                std::iter::repeat(false)
                    .take(missing_len)
                    .chain(nulls.iter()),
            )
        } else {
            #[allow(clippy::from_iter_instead_of_collect)]
            NullBuffer::from_iter(
                std::iter::repeat(false)
                    .take(missing_len)
                    .chain(std::iter::repeat(true).take(list_array.len())),
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

    let offsets = OffsetBuffer::from_lengths(std::iter::repeat(0).take(len));

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
    #[allow(clippy::disallowed_methods)] // that's the whole point
    arrow::compute::concat(arrays)
    // TODO(#3741): call .shrink_to_fit on the result
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
        array.len(), filter.len(),
        "the length of the filter must match the length of the array (the underlying kernel will panic otherwise)",
    );
    debug_assert!(
        filter.nulls().is_none(),
        "filter masks with nulls bits are technically valid, but generally a sign that something went wrong",
    );

    #[allow(clippy::disallowed_methods)] // that's the whole point
    #[allow(clippy::unwrap_used)]
    arrow::compute::filter(array, filter)
        // Unwrap: this literally cannot fail.
        .unwrap()
        .as_any()
        .downcast_ref::<A>()
        // Unwrap: that's initial type that we got.
        .unwrap()
        .clone()
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
            #[allow(clippy::unwrap_used)]
            return array
                .clone()
                .as_any()
                .downcast_ref::<A>()
                // Unwrap: that's initial type that we got.
                .unwrap()
                .clone();
        }
    }

    #[allow(clippy::disallowed_methods)] // that's the whole point
    #[allow(clippy::unwrap_used)]
    arrow::compute::take(array, indices, Default::default())
        // Unwrap: this literally cannot fail.
        .unwrap()
        .as_any()
        .downcast_ref::<A>()
        // Unwrap: that's initial type that we got.
        .unwrap()
        .clone()
}
