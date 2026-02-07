use std::ffi::c_char;

use crate::{CError, CErrorCode};

// ---

#[expect(unsafe_code)]
#[expect(clippy::result_large_err)]
pub fn try_ptr_as_ref<T>(ptr: *const T, argument_name: &str) -> Result<&T, CError> {
    let ptr = unsafe { ptr.as_ref() };
    if let Some(ptr) = ptr {
        Ok(ptr)
    } else {
        Err(CError::unexpected_null(argument_name))
    }
}

#[expect(unsafe_code)]
#[expect(clippy::result_large_err)]
pub fn try_ptr_as_slice<T>(
    ptr: *const T,
    length: u32,
    argument_name: &str,
) -> Result<&[T], CError> {
    try_ptr_as_ref(ptr, argument_name)?;
    Ok(unsafe { std::slice::from_raw_parts(ptr.cast::<T>(), length as usize) })
}

/// Tries to convert a [`c_char`] pointer to a string, raises an error if the pointer is null or it can't be converted to a string.
#[expect(unsafe_code)]
#[expect(clippy::result_large_err)]
pub fn try_char_ptr_as_str(
    ptr: *const c_char,
    string_length_in_bytes: u32,
    argument_name: &str,
) -> Result<&str, CError> {
    try_ptr_as_ref(ptr, argument_name)?;

    let byte_slice =
        unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), string_length_in_bytes as usize) };

    // Make sure there's no null-terminator within that range.
    // We're strict and fail then because the input doesn't match our expectations.
    // Alternatively, we could cut the string short at the nullterminator.
    if let Some(null_terminator_position) = byte_slice.iter().position(|b| *b == b'\0') {
        return Err(CError::new(
            CErrorCode::InvalidStringArgument,
            &format!(
                "Argument {argument_name:?} was specified to be a string with length {string_length_in_bytes}, but there is an unexpected null-terminator at position {null_terminator_position}."
            ),
        ));
    }

    // The byte slice is
    match std::str::from_utf8(byte_slice) {
        Ok(str) => Ok(str),
        Err(utf8_error) => Err(CError::invalid_str_argument(argument_name, utf8_error)),
    }
}
