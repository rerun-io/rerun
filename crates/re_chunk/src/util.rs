use arrow2::{
    array::{Array as ArrowArray, ListArray as ArrowListArray},
    bitmap::Bitmap as ArrowBitmap,
    offset::Offsets as ArrowOffsets,
};
use itertools::Itertools as _;

// ---

/// Create a sparse list-array out of an array of arrays.
///
/// All arrays must have the same datatype.
///
/// Returns `None` if `arrays` is empty.
pub fn arrays_to_list_array(arrays: &[Option<&dyn ArrowArray>]) -> Option<Box<dyn ArrowArray>> {
    let arrays_dense = arrays.iter().flatten().copied().collect_vec();

    if arrays_dense.is_empty() {
        return None;
    }

    let data = arrow2::compute::concatenate::concatenate(&arrays_dense)
        .map_err(|err| {
            re_log::warn_once!("failed to concatenate arrays: {err}");
            err
        })
        .ok()?;

    let datatype = arrays_dense
        .first()
        .map(|array| array.data_type().clone())?;
    debug_assert!(arrays_dense
        .iter()
        .all(|array| *array.data_type() == datatype));
    let datatype = ArrowListArray::<i32>::default_datatype(datatype);

    #[allow(clippy::unwrap_used)] // yes, there are indeed lengths
    let offsets = ArrowOffsets::try_from_lengths(
        arrays
            .iter()
            .map(|array| array.map_or(0, |array| array.len())),
    )
    .unwrap();

    #[allow(clippy::from_iter_instead_of_collect)]
    let validity = ArrowBitmap::from_iter(arrays.iter().map(Option::is_some));

    Some(ArrowListArray::<i32>::new(datatype, offsets.into(), data, validity.into()).boxed())
}
