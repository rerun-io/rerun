use super::TextLogLevel;

impl TextLogLevel {
    /// Designates catastrophic failures.
    pub const CRITICAL: &'static str = "CRITICAL";

    /// Designates very serious errors.
    pub const ERROR: &'static str = "ERROR";

    /// Designates hazardous situations.
    pub const WARN: &'static str = "WARN";

    /// Designates useful information.
    pub const INFO: &'static str = "INFO";

    /// Designates lower priority information.
    pub const DEBUG: &'static str = "DEBUG";

    /// Designates very low priority, often extremely verbose, information.
    pub const TRACE: &'static str = "TRACE";

    /// The log level as a string slice, e.g. "INFO".
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<TextLogLevel> for String {
    #[inline]
    fn from(value: TextLogLevel) -> Self {
        value.as_str().to_owned()
    }
}

impl AsRef<str> for TextLogLevel {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for TextLogLevel {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl Default for TextLogLevel {
    #[inline]
    fn default() -> Self {
        Self::INFO.to_owned().into()
    }
}
