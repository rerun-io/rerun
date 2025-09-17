use re_log_types::TimestampFormat;

/// Wrapper structure to associate a [`TimestampFormat`] with a reference.
///
/// This is used to implement [`re_ui::SyntaxHighlighting`] to types which may need to display
/// timestamps.
///
/// Note: this can't be moved to `re_log_types` along with `TimestampFormat`, because then we cannot
/// implement traits for it.
pub struct TimestampFormatted<'a, T> {
    pub inner: &'a T,
    pub timestamp_format: TimestampFormat,
}

impl<'a, T> TimestampFormatted<'a, T> {
    pub fn new(value: &'a T, timestamp_format: TimestampFormat) -> Self {
        TimestampFormatted {
            inner: value,
            timestamp_format,
        }
    }

    /// Apply the same timestamp format to another reference.
    pub fn convert<'b, U>(&'a self, value: &'b U) -> TimestampFormatted<'b, U> {
        TimestampFormatted {
            inner: value,
            timestamp_format: self.timestamp_format,
        }
    }
}
