use arrow2::{
    array::{
        Array as Arrow2Array, BooleanArray as Arrow2BooleanArray,
        DictionaryArray as ArrowDictionaryArray, ListArray as ArrowListArray,
        PrimitiveArray as Arrow2PrimitiveArray,
    },
    bitmap::Bitmap as ArrowBitmap,
    datatypes::DataType as Arrow2Datatype,
    offset::Offsets as ArrowOffsets,
};
use itertools::Itertools as _;

// ---------------------------------------------------------------------------------

/// Downcast an arrow array to another array, without having to go via `Any`.
///
/// This is shorter, but also better: it means we don't accidentally downcast
/// an arrow2 array to an arrow1 array, or vice versa.
pub trait Arrow2ArrayDowncastRef {
    /// Downcast an arrow array to another array, without having to go via `Any`.
    ///
    /// This is shorter, but also better: it means we don't accidentally downcast
    /// an arrow2 array to an arrow1 array, or vice versa.
    fn downcast_array2_ref<T: Arrow2Array + 'static>(&self) -> Option<&T>;
}

impl Arrow2ArrayDowncastRef for dyn Arrow2Array {
    fn downcast_array2_ref<T: Arrow2Array + 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref()
    }
}

impl<A> Arrow2ArrayDowncastRef for A
where
    A: Arrow2Array,
{
    fn downcast_array2_ref<T: Arrow2Array + 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref()
    }
}

// ---------------------------------------------------------------------------------

/// Returns true if the given `list_array` is semantically empty.
///
/// Semantic emptiness is defined as either one of these:
/// * The list is physically empty (literally no data).
/// * The list only contains null entries, or empty arrays, or a mix of both.
pub fn is_list_array_semantically_empty(list_array: &ArrowListArray<i32>) -> bool {
    let is_physically_empty = || list_array.is_empty();

    let is_all_nulls = || {
        list_array
            .validity()
            .is_some_and(|bitmap| bitmap.unset_bits() == list_array.len())
    };

    let is_all_empties = || list_array.offsets().lengths().all(|len| len == 0);

    let is_a_mix_of_nulls_and_empties =
        || list_array.iter().flatten().all(|array| array.is_empty());

    is_physically_empty() || is_all_nulls() || is_all_empties() || is_a_mix_of_nulls_and_empties()
}

/// Create a sparse list-array out of an array of arrays.
///
/// All arrays must have the same datatype.
///
/// Returns `None` if `arrays` is empty.
#[inline]
pub fn arrays_to_list_array_opt(
    arrays: &[Option<&dyn Arrow2Array>],
) -> Option<ArrowListArray<i32>> {
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
    array_datatype: Arrow2Datatype,
    arrays: &[Option<&dyn Arrow2Array>],
) -> Option<ArrowListArray<i32>> {
    let arrays_dense = arrays.iter().flatten().copied().collect_vec();

    let data = if arrays_dense.is_empty() {
        arrow2::array::new_empty_array(array_datatype.clone())
    } else {
        re_tracing::profile_scope!("concatenate", arrays_dense.len().to_string());
        concat_arrays(&arrays_dense)
            .map_err(|err| {
                re_log::warn_once!("failed to concatenate arrays: {err}");
                err
            })
            .ok()?
    };

    let datatype = ArrowListArray::<i32>::default_datatype(array_datatype);

    #[allow(clippy::unwrap_used)] // yes, these are indeed lengths
    let offsets = ArrowOffsets::try_from_lengths(
        arrays
            .iter()
            .map(|array| array.map_or(0, |array| array.len())),
    )
    .unwrap();

    #[allow(clippy::from_iter_instead_of_collect)]
    let validity = ArrowBitmap::from_iter(arrays.iter().map(Option::is_some));

    Some(ArrowListArray::<i32>::new(
        datatype,
        offsets.into(),
        data,
        validity.into(),
    ))
}

/// Create a sparse dictionary-array out of an array of (potentially) duplicated arrays.
///
/// The `Idx` is used as primary key to drive the deduplication process.
/// Returns `None` if any of the specified `arrays` doesn't match the given `array_datatype`.
///
/// Returns an empty dictionary if `arrays` is empty.
//
// TODO(cmc): Ideally I would prefer to just use the array's underlying pointer as primary key, but
// this has proved extremely brittle in practice. Maybe once we move to arrow-rs.
// TODO(cmc): A possible improvement would be to pick the smallest key datatype possible based
// on the cardinality of the input arrays.
pub fn arrays_to_dictionary<Idx: Copy + Eq>(
    array_datatype: &Arrow2Datatype,
    arrays: &[Option<(Idx, &dyn Arrow2Array)>],
) -> Option<ArrowDictionaryArray<i32>> {
    // Dedupe the input arrays based on the given primary key.
    let arrays_dense_deduped = arrays
        .iter()
        .flatten()
        .copied()
        .dedup_by(|(lhs_index, _), (rhs_index, _)| lhs_index == rhs_index)
        .map(|(_index, array)| array)
        .collect_vec();

    // Compute the keys for the final dictionary, using that same primary key.
    let keys = {
        let mut cur_key = 0i32;
        arrays
            .iter()
            .dedup_by_with_count(|lhs, rhs| {
                lhs.map(|(index, _)| index) == rhs.map(|(index, _)| index)
            })
            .flat_map(|(count, value)| {
                if value.is_some() {
                    let keys = std::iter::repeat(Some(cur_key)).take(count);
                    cur_key += 1;
                    keys
                } else {
                    std::iter::repeat(None).take(count)
                }
            })
            .collect_vec()
    };

    // Concatenate the underlying data as usual, except only the _unique_ values!
    // We still need the underlying data to be a list-array, so the dictionary's keys can index
    // into this list-array.
    let data = if arrays_dense_deduped.is_empty() {
        arrow2::array::new_empty_array(array_datatype.clone())
    } else {
        let values = concat_arrays(&arrays_dense_deduped)
            .map_err(|err| {
                re_log::warn_once!("failed to concatenate arrays: {err}");
                err
            })
            .ok()?;

        #[allow(clippy::unwrap_used)] // yes, these are indeed lengths
        let offsets =
            ArrowOffsets::try_from_lengths(arrays_dense_deduped.iter().map(|array| array.len()))
                .unwrap();

        ArrowListArray::<i32>::new(array_datatype.clone(), offsets.into(), values, None).to_boxed()
    };

    let datatype = Arrow2Datatype::Dictionary(
        arrow2::datatypes::IntegerType::Int32,
        std::sync::Arc::new(array_datatype.clone()),
        true, // is_sorted
    );

    // And finally we build our dictionary, which indexes into our concatenated list-array of
    // unique values.
    ArrowDictionaryArray::try_new(
        datatype,
        Arrow2PrimitiveArray::<i32>::from(keys),
        data.to_boxed(),
    )
    .ok()
}

/// Given a sparse `ArrowListArray` (i.e. an array with a validity bitmap that contains at least
/// one falsy value), returns a dense `ArrowListArray` that only contains the non-null values from
/// the original list.
///
/// This is a no-op if the original array is already dense.
pub fn sparse_list_array_to_dense_list_array(
    list_array: &ArrowListArray<i32>,
) -> ArrowListArray<i32> {
    if list_array.is_empty() {
        return list_array.clone();
    }

    let is_empty = list_array
        .validity()
        .is_some_and(|validity| validity.is_empty());
    if is_empty {
        return list_array.clone();
    }

    #[allow(clippy::unwrap_used)] // yes, these are indeed lengths
    let offsets =
        ArrowOffsets::try_from_lengths(list_array.iter().flatten().map(|array| array.len()))
            .unwrap();

    ArrowListArray::<i32>::new(
        list_array.data_type().clone(),
        offsets.into(),
        list_array.values().clone(),
        None,
    )
}

/// Create a new `ListArray` of target length by appending null values to its back.
///
/// This will share the same child data array buffer, but will create new offset and validity buffers.
pub fn pad_list_array_back(
    list_array: &ArrowListArray<i32>,
    target_len: usize,
) -> ArrowListArray<i32> {
    let missing_len = target_len.saturating_sub(list_array.len());
    if missing_len == 0 {
        return list_array.clone();
    }

    let datatype = list_array.data_type().clone();

    let offsets = {
        #[allow(clippy::unwrap_used)] // yes, these are indeed lengths
        ArrowOffsets::try_from_lengths(
            list_array
                .iter()
                .map(|array| array.map_or(0, |array| array.len()))
                .chain(std::iter::repeat(0).take(missing_len)),
        )
        .unwrap()
    };

    let values = list_array.values().clone();

    let validity = {
        if let Some(validity) = list_array.validity() {
            #[allow(clippy::from_iter_instead_of_collect)]
            ArrowBitmap::from_iter(
                validity
                    .iter()
                    .chain(std::iter::repeat(false).take(missing_len)),
            )
        } else {
            #[allow(clippy::from_iter_instead_of_collect)]
            ArrowBitmap::from_iter(
                std::iter::repeat(true)
                    .take(list_array.len())
                    .chain(std::iter::repeat(false).take(missing_len)),
            )
        }
    };

    ArrowListArray::new(datatype, offsets.into(), values, Some(validity))
}

/// Create a new `ListArray` of target length by appending null values to its front.
///
/// This will share the same child data array buffer, but will create new offset and validity buffers.
pub fn pad_list_array_front(
    list_array: &ArrowListArray<i32>,
    target_len: usize,
) -> ArrowListArray<i32> {
    let missing_len = target_len.saturating_sub(list_array.len());
    if missing_len == 0 {
        return list_array.clone();
    }

    let datatype = list_array.data_type().clone();

    let offsets = {
        #[allow(clippy::unwrap_used)] // yes, these are indeed lengths
        ArrowOffsets::try_from_lengths(
            std::iter::repeat(0).take(missing_len).chain(
                list_array
                    .iter()
                    .map(|array| array.map_or(0, |array| array.len())),
            ),
        )
        .unwrap()
    };

    let values = list_array.values().clone();

    let validity = {
        if let Some(validity) = list_array.validity() {
            #[allow(clippy::from_iter_instead_of_collect)]
            ArrowBitmap::from_iter(
                std::iter::repeat(false)
                    .take(missing_len)
                    .chain(validity.iter()),
            )
        } else {
            #[allow(clippy::from_iter_instead_of_collect)]
            ArrowBitmap::from_iter(
                std::iter::repeat(false)
                    .take(missing_len)
                    .chain(std::iter::repeat(true).take(list_array.len())),
            )
        }
    };

    ArrowListArray::new(datatype, offsets.into(), values, Some(validity))
}

/// Returns a new [`ArrowListArray`] with len `entries`.
///
/// Each entry will be an empty array of the given `child_datatype`.
pub fn new_list_array_of_empties(
    child_datatype: Arrow2Datatype,
    len: usize,
) -> ArrowListArray<i32> {
    let empty_array = arrow2::array::new_empty_array(child_datatype);

    #[allow(clippy::unwrap_used)] // yes, these are indeed lengths
    let offsets = ArrowOffsets::try_from_lengths(std::iter::repeat(0).take(len)).unwrap();

    ArrowListArray::<i32>::new(
        ArrowListArray::<i32>::default_datatype(empty_array.data_type().clone()),
        offsets.into(),
        empty_array.to_boxed(),
        None,
    )
}

/// Applies a [concatenate] kernel to the given `arrays`.
///
/// Early outs where it makes sense (e.g. `arrays.len() == 1`).
///
/// Returns an error if the arrays don't share the exact same datatype.
///
/// [concatenate]: arrow2::compute::concatenate::concatenate
pub fn concat_arrays(arrays: &[&dyn Arrow2Array]) -> arrow2::error::Result<Box<dyn Arrow2Array>> {
    if arrays.len() == 1 {
        return Ok(arrays[0].to_boxed());
    }

    #[allow(clippy::disallowed_methods)] // that's the whole point
    arrow2::compute::concatenate::concatenate(arrays)
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
/// [filter]: arrow2::compute::filter::filter
pub fn filter_array<A: Arrow2Array + Clone>(array: &A, filter: &Arrow2BooleanArray) -> A {
    assert_eq!(
        array.len(), filter.len(),
        "the length of the filter must match the length of the array (the underlying kernel will panic otherwise)",
    );
    debug_assert!(
        filter.validity().is_none(),
        "filter masks with validity bits are technically valid, but generally a sign that something went wrong",
    );

    #[allow(clippy::disallowed_methods)] // that's the whole point
    #[allow(clippy::unwrap_used)]
    arrow2::compute::filter::filter(array, filter)
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
/// [take]: arrow2::compute::take::take
//
// TODO(cmc): in an ideal world, a `take` kernel should merely _slice_ the data and avoid any allocations/copies
// where possible (e.g. list-arrays).
// That is not possible with vanilla `ListArray`s since they don't expose any way to encode optional lengths,
// in addition to offsets.
// For internal stuff, we could perhaps provide a custom implementation that returns a `DictionaryArray` instead?
pub fn take_array<A: Arrow2Array + Clone, O: arrow2::types::Index>(
    array: &A,
    indices: &Arrow2PrimitiveArray<O>,
) -> A {
    debug_assert!(
        indices.validity().is_none(),
        "index arrays with validity bits are technically valid, but generally a sign that something went wrong",
    );

    if indices.len() == array.len() {
        let indices = indices.values().as_slice();

        let starts_at_zero = || indices[0] == O::zero();
        let is_consecutive = || {
            indices
                .windows(2)
                .all(|values| values[1] == values[0] + O::one())
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
    arrow2::compute::take::take(array, indices)
        // Unwrap: this literally cannot fail.
        .unwrap()
        .as_any()
        .downcast_ref::<A>()
        // Unwrap: that's initial type that we got.
        .unwrap()
        .clone()
}
