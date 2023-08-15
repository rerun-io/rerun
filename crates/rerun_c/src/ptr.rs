use std::ffi::{c_char, CStr};

use crate::CError;

#[allow(unsafe_code)]
pub fn try_ptr_as_ref<T>(ptr: *const T, error: *mut CError, argument_name: &str) -> Option<&T> {
    let ptr = unsafe { ptr.as_ref() };
    if let Some(ptr) = ptr {
        Some(ptr)
    } else {
        CError::unexpected_null(error, argument_name);
        None
    }
}

/// Tries to convert a c_char pointer to a string, raises an error if the pointer is null or it can't be converted to a string.
#[allow(unsafe_code)]
pub fn try_char_ptr_as_str(
    ptr: *const c_char,
    error: *mut CError,
    argument_name: &str,
) -> Option<&str> {
    if try_ptr_as_ref(ptr, error, argument_name).is_some() {
        match unsafe { CStr::from_ptr(ptr) }.to_str() {
            Ok(str) => Some(str),
            Err(utf8_error) => {
                CError::invalid_str_argument(error, argument_name, utf8_error);
                None
            }
        }
    } else {
        None
    }
}
