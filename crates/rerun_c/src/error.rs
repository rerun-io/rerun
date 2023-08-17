use crate::{CError, CErrorCode};

impl CError {
    const OK: CError = CError {
        code: CErrorCode::Ok,
        message: [0; 512],
    };

    #[allow(unsafe_code)]
    pub(crate) fn write_error(error: *mut CError, code: CErrorCode, message: &str) {
        let error = unsafe { error.as_mut() };
        let Some(error) = error else {
            return;
        };

        error.code = code;

        // Copy string character by character.
        // Ensure that when truncating is necessary, we don't truncate in the middle of a UTF-8 character!
        let mut bytes_next = 0;
        for c in message.chars() {
            if bytes_next + c.len_utf8() >= error.message.len() {
                re_log::warn_once!("Error message was too long for C error description buffer. Full message\n{message}");
                break;
            }

            let mut bytes = [0; 4];
            c.encode_utf8(&mut bytes);

            for byte in &bytes[..c.len_utf8()] {
                error.message[bytes_next] = *byte as _;
                bytes_next += 1;
            }
        }

        // Fill the rest with nulls.
        for byte in &mut error.message[bytes_next..] {
            *byte = 0;
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
            &format!("Failed to interpret argument {argument_name:?} as a UTF-8: {utf8_error}",),
        );
    }

    pub fn invalid_recording_stream_handle(error: *mut CError) {
        CError::write_error(
            error,
            CErrorCode::InvalidRecordingStreamHandle,
            "Recording stream handle does not point to an existing recording stream.",
        );
    }

    #[allow(unsafe_code)]
    pub(crate) fn set_ok(error: *mut CError) {
        if let Some(error) = unsafe { error.as_mut() } {
            *error = CError::OK.clone();
        };
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::{c_char, CStr};

    use crate::{CError, CErrorCode};

    #[test]
    #[allow(unsafe_code)]
    fn write_error_handles_message_overflow() {
        let mut error = CError::OK.clone();

        // With ASCII character.
        let num_expected_bytes = error.message.len() - 1;
        let description = "a".repeat(1024);
        CError::write_error(&mut error as *mut CError, CErrorCode::Ok, &description);
        assert_eq!(
            unsafe { CStr::from_ptr(&error.message as *const c_char) }.to_str(),
            Ok(&description[..num_expected_bytes])
        );

        // With 2 byte UTF8 character
        let num_expected_bytes = ((error.message.len() - 1) / 2) * 2;
        let description = "œ".repeat(1024);
        CError::write_error(&mut error as *mut CError, CErrorCode::Ok, &description);
        assert_eq!(
            unsafe { CStr::from_ptr(&error.message as *const c_char) }.to_str(),
            Ok(&description[..num_expected_bytes])
        );

        // With 3 byte UTF8 character
        let num_expected_bytes = ((error.message.len() - 1) / 3) * 3;
        let description = "∂".repeat(1024);
        CError::write_error(&mut error as *mut CError, CErrorCode::Ok, &description);
        assert_eq!(
            unsafe { CStr::from_ptr(&error.message as *const c_char) }.to_str(),
            Ok(&description[..num_expected_bytes])
        );

        // With 4 byte UTF8 character
        let num_expected_bytes = ((error.message.len() - 1) / 4) * 4;
        let description = "😀".repeat(1024);
        CError::write_error(&mut error as *mut CError, CErrorCode::Ok, &description);
        assert_eq!(
            unsafe { CStr::from_ptr(&error.message as *const c_char) }.to_str(),
            Ok(&description[..num_expected_bytes])
        );
    }
}
