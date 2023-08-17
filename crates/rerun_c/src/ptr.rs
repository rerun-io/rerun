use std::ffi::{c_char, CStr};

use crate::CError;

#[allow(unsafe_code)]
#[allow(clippy::result_large_err)]
pub fn try_ptr_as_ref<T>(ptr: *const T, argument_name: &str) -> Result<&T, CError> {
    let ptr = unsafe { ptr.as_ref() };
    if let Some(ptr) = ptr {
        Ok(ptr)
    } else {
        Err(CError::unexpected_null(argument_name))
    }
}

/// Tries to convert a [`c_char`] pointer to a string, raises an error if the pointer is null or it can't be converted to a string.
#[allow(unsafe_code)]
#[allow(clippy::result_large_err)]
pub fn try_char_ptr_as_str(ptr: *const c_char, argument_name: &str) -> Result<&str, CError> {
    try_ptr_as_ref(ptr, argument_name)?;

    match unsafe { CStr::from_ptr(ptr) }.to_str() {
        Ok(str) => Ok(str),
        Err(utf8_error) => Err(CError::invalid_str_argument(argument_name, utf8_error)),
    }
}
