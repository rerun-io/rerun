use arrow2::{
    array::{
        Array as ArrowArray, BooleanArray as ArrowBooleanArray,
        DictionaryArray as ArrowDictionaryArray, ListArray as ArrowListArray,
        PrimitiveArray as ArrowPrimitiveArray,
    },
    bitmap::Bitmap as ArrowBitmap,
    datatypes::DataType as ArrowDatatype,
    offset::Offsets as ArrowOffsets,
};
use itertools::Itertools;

// ---

/// Create a sparse list-array out of an array of arrays.
///
/// All arrays must have the same datatype.
///
/// Returns `None` if `arrays` is empty.
#[inline]
pub fn arrays_to_list_array_opt(arrays: &[Option<&dyn ArrowArray>]) -> Option<ArrowListArray<i32>> {
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
    array_datatype: ArrowDatatype,
    arrays: &[Option<&dyn ArrowArray>],
) -> Option<ArrowListArray<i32>> {
    let arrays_dense = arrays.iter().flatten().copied().collect_vec();

    let data = if arrays_dense.is_empty() {
        arrow2::array::new_empty_array(array_datatype.clone())
    } else {
        arrow2::compute::concatenate::concatenate(&arrays_dense)
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
    array_datatype: ArrowDatatype,
    arrays: &[Option<(Idx, &dyn ArrowArray)>],
) -> Option<ArrowDictionaryArray<u32>> {
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
        let mut cur_key = 0u32;
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
    let data = if arrays_dense_deduped.is_empty() {
        arrow2::array::new_empty_array(array_datatype.clone())
    } else {
        arrow2::compute::concatenate::concatenate(&arrays_dense_deduped)
            .map_err(|err| {
                re_log::warn_once!("failed to concatenate arrays: {err}");
                err
            })
            .ok()?
    };

    // We still need the underlying data to be a list-array, so the dictionary's keys can index
    // into this list-array.
    let data = {
        let datatype = ArrowListArray::<i32>::default_datatype(array_datatype);

        #[allow(clippy::unwrap_used)] // yes, these are indeed lengths
        let offsets =
            ArrowOffsets::try_from_lengths(arrays_dense_deduped.iter().map(|array| array.len()))
                .unwrap();

        ArrowListArray::<i32>::new(datatype, offsets.into(), data, None)
    };

    let datatype = ArrowDatatype::Dictionary(
        arrow2::datatypes::IntegerType::UInt32,
        std::sync::Arc::new(data.data_type().clone()),
        true, // is_sorted
    );

    // And finally we build our dictionary, which indexes into our concatenated list-array of
    // unique values.
    ArrowDictionaryArray::try_new(
        datatype,
        ArrowPrimitiveArray::<u32>::from(keys),
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
        .map_or(false, |validity| validity.is_empty());
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

/// Applies a filter kernel to the given `array`.
///
/// Takes care of up- and down-casting the data back and forth on behalf of the caller.
pub fn filter_array<A: ArrowArray + Clone>(array: &A, filter: &ArrowBooleanArray) -> A {
    debug_assert!(filter.validity().is_none()); // just for good measure

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
