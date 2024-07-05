use arrow2::{
    array::{Array as ArrowArray, BooleanArray as ArrowBooleanArray, ListArray as ArrowListArray},
    bitmap::Bitmap as ArrowBitmap,
    datatypes::DataType as ArrowDataType,
    offset::Offsets as ArrowOffsets,
};
use itertools::Itertools as _;

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
    array_datatype: ArrowDataType,
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

/// Applies a filter kernel to the given `array`.
///
/// Takes care of up- and down-casting the data back and forth on behalf of the caller.
pub fn filter_array<A: ArrowArray + Clone>(array: &A, filter: &ArrowBooleanArray) -> A {
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
