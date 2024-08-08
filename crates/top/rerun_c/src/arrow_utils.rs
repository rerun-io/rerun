use crate::{CError, CErrorCode};

/// Converts a C-FFI arrow array into a Rust component batch, taking ownership of the underlying arrow data. ///
///
/// Safety:
/// This must only be ever called once for a given ffi array.
/// Conceptually, this takes ownership of the array, i.e. this should really be a move operation,
/// but since we have typically pass c arrays (ptr + length), we can't actually move out data.
#[allow(unsafe_code)]
#[allow(clippy::result_large_err)]
pub unsafe fn arrow_array_from_c_ffi(
    array: &arrow2::ffi::ArrowArray,
    datatype: arrow2::datatypes::DataType,
) -> Result<Box<dyn arrow2::array::Array>, CError> {
    // Arrow2 implements drop for ArrowArray and ArrowSchema.
    //
    // Therefore, for things to work correctly we have to take ownership of the array!
    // All methods passing arrow arrays through our C interface are documented to take ownership of the component batch.
    // I.e. the user should NOT call `release`.
    //
    // This makes sense because from here on out we want to manage the lifetime of the underlying schema and array data
    // from the rust side.
    unsafe { arrow2::ffi::import_array_from_c(std::ptr::read(array), datatype) }.map_err(|err| {
        CError::new(
            CErrorCode::ArrowFfiArrayImportError,
            &format!("Failed to import ffi array: {err}"),
        )
    })
}
