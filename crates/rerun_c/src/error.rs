use crate::{CError, CErrorCode};

impl CError {
    pub fn write_error(error: *mut CError, code: CErrorCode, message: &str) {
        #[allow(unsafe_code)]
        let error = unsafe { error.as_mut() };
        let Some(error) = error else {
            return;
        };

        error.code = code;

        let bytes = message.bytes();
        let message_byte_length_excluding_null = bytes.len().min(error.message.len() - 1);

        // If we have to truncate the error message log a warning.
        // (we don't know how critical it is, but we can't just swallow this silently!)
        if bytes.len() < message_byte_length_excluding_null {
            re_log::warn_once!("CError message was too long. Full message\n{message}");
        }

        // Copy over string and null out the rest.
        for (left, right) in error.message.iter_mut().zip(
            message
                .bytes()
                .take(message_byte_length_excluding_null)
                .chain(std::iter::repeat(0)),
        ) {
            *left = right as std::ffi::c_char;
        }
    }

    pub fn unexpected_null(error: *mut CError, argument_name: &str) {
        Self::write_error(
            error,
            CErrorCode::UnexpectedNullArgument,
            &format!("Unexpected null passed for argument '{argument_name:?}'"),
        );
    }

    pub fn invalid_str_argument(
        error: *mut CError,
        argument_name: &str,
        utf8_error: std::str::Utf8Error,
    ) {
        CError::write_error(
            error,
            CErrorCode::InvalidStringArgument,
            &format!("Failed to interpret argument '{argument_name:?}' as a UTF-8: {utf8_error}",),
        );
    }

    pub fn set_ok(error: *mut CError) {
        CError::write_error(error, CErrorCode::Ok, "success");
    }
}
