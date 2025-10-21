use crate::{CError, CErrorCode};

impl CError {
    /// The maximum size in bytes of the [`CError::message`] field.
    ///
    /// Error message larger than this value will be automatically truncated.
    //
    // NOTE: You must update `rr_error.description` too if you modify this value.
    pub const MAX_MESSAGE_SIZE_BYTES: usize = 2048;

    pub const OK: Self = Self {
        code: CErrorCode::Ok,
        message: [0; Self::MAX_MESSAGE_SIZE_BYTES],
    };

    pub fn new(code: CErrorCode, message: &str) -> Self {
        let mut message_c = [0; Self::MAX_MESSAGE_SIZE_BYTES];

        // Copy string character by character.
        // Ensure that when truncating is necessary, we don't truncate in the middle of a UTF-8 character!
        let mut bytes_next = 0;
        for c in message.chars() {
            if bytes_next + c.len_utf8() >= message_c.len() {
                re_log::warn_once!(
                    "Error message was too long for C error description buffer. Full message\n{message}"
                );
                break;
            }

            let mut bytes = [0; 4];
            c.encode_utf8(&mut bytes);

            for byte in &bytes[..c.len_utf8()] {
                // `c_char` is something different depending on platforms, and this is needed for
                // when it's the same as `u8`.
                #[allow(trivial_numeric_casts, clippy::allow_attributes)]
                #[expect(clippy::cast_possible_wrap)] // intentional!
                {
                    message_c[bytes_next] = *byte as _;
                }
                bytes_next += 1;
            }
        }

        // Fill the rest with nulls.
        for byte in &mut message_c[bytes_next..] {
            *byte = 0;
        }

        Self {
            code,
            message: message_c,
        }
    }

    pub fn unexpected_null(parameter_name: &str) -> Self {
        Self::new(
            CErrorCode::UnexpectedNullArgument,
            &format!("Unexpected null passed for parameter '{parameter_name:?}'"),
        )
    }

    pub fn invalid_str_argument(parameter_name: &str, utf8_error: std::str::Utf8Error) -> Self {
        Self::new(
            CErrorCode::InvalidStringArgument,
            &format!("Argument {parameter_name:?} is not valid UTF-8: {utf8_error}",),
        )
    }

    pub fn invalid_recording_stream_handle() -> Self {
        Self::new(
            CErrorCode::InvalidRecordingStreamHandle,
            "Recording stream handle does not point to an existing recording stream.",
        )
    }

    #[expect(unsafe_code)]
    pub(crate) fn write_error(self, error: *mut Self) {
        if let Some(error) = unsafe { error.as_mut() } {
            *error = self;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, c_char};

    use crate::{CError, CErrorCode};

    #[test]
    fn write_error_handles_message_overflow() {
        #![expect(clippy::ref_as_ptr)]
        #![expect(unsafe_code)]

        // With ASCII character.
        let description = "a".repeat(CError::MAX_MESSAGE_SIZE_BYTES * 2);
        let error = CError::new(CErrorCode::Ok, &description);
        let num_expected_bytes = error.message.len() - 1;
        assert_eq!(
            unsafe { CStr::from_ptr(&error.message as *const c_char) }.to_str(),
            Ok(&description[..num_expected_bytes])
        );

        // With 2 byte UTF8 character
        let description = "œ".repeat(CError::MAX_MESSAGE_SIZE_BYTES * 2);
        let error = CError::new(CErrorCode::Ok, &description);
        let num_expected_bytes = ((error.message.len() - 1) / 2) * 2;
        assert_eq!(
            unsafe { CStr::from_ptr(&error.message as *const c_char) }.to_str(),
            Ok(&description[..num_expected_bytes])
        );

        // With 3 byte UTF8 character
        let description = "∂".repeat(CError::MAX_MESSAGE_SIZE_BYTES * 2);
        let error = CError::new(CErrorCode::Ok, &description);
        let num_expected_bytes = ((error.message.len() - 1) / 3) * 3;
        assert_eq!(
            unsafe { CStr::from_ptr(&error.message as *const c_char) }.to_str(),
            Ok(&description[..num_expected_bytes])
        );

        // With 4 byte UTF8 character
        let description = "😀".repeat(CError::MAX_MESSAGE_SIZE_BYTES * 2);
        let error = CError::new(CErrorCode::Ok, &description);
        let num_expected_bytes = ((error.message.len() - 1) / 4) * 4;
        assert_eq!(
            unsafe { CStr::from_ptr(&error.message as *const c_char) }.to_str(),
            Ok(&description[..num_expected_bytes])
        );
    }
}
