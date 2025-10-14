use arrow::ffi::FFI_ArrowArray;

use crate::{CError, CErrorCode};

/// Converts a C-FFI arrow array into a Rust component batch, taking ownership of the underlying arrow data.
///
/// ### Safety
/// This struct assumes that the incoming data agrees with the C data interface.
#[expect(unsafe_code)]
#[expect(clippy::result_large_err)]
pub unsafe fn arrow_array_from_c_ffi(
    array: FFI_ArrowArray,
    datatype: arrow::datatypes::DataType,
) -> Result<arrow::array::ArrayRef, CError> {
    // arrow-rs implements `Drop` for `FFI_ArrowArray`.
    //
    // Therefore, for things to work correctly we have to take ownership of the array!
    // All methods passing arrow arrays through our C interface are documented to take ownership of the component batch.
    // I.e. the user should NOT call `release`.
    //
    // This makes sense because from here on out we want to manage the lifetime of the underlying schema and array data
    // from the rust side.
    unsafe { arrow::ffi::from_ffi_and_data_type(array, datatype) }
        .map(arrow::array::make_array)
        .map_err(|err| {
            CError::new(
                CErrorCode::ArrowFfiArrayImportError,
                &format!("Failed to import ffi array: {err}"),
            )
        })
}
