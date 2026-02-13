re_string_interner::declare_new_type!(
    /// The name of a timeline. Often something like `"log_time"` or `"frame_nr"`.
    ///
    /// This uniquely identifies a timeline.
    #[cfg_attr(feature = "serde", derive(::serde::Deserialize, ::serde::Serialize))]
    pub struct TimelineName;
);

impl TimelineName {
    /// The log time timeline to which all API functions will always log.
    ///
    /// This timeline is automatically maintained by the SDKs and captures the wall-clock time at
    /// which point the data was logged (according to the client's wall-clock).
    #[inline]
    pub fn log_time() -> Self {
        Self::new("log_time")
    }

    /// The log tick timeline to which all API functions will always log.
    ///
    /// This timeline is automatically maintained by the SDKs and captures the logging tick at
    /// which point the data was logged.
    /// The logging tick is monotically incremented each time the client calls one of the logging
    /// methods on a `RecordingStream`.
    #[inline]
    pub fn log_tick() -> Self {
        Self::new("log_tick")
    }
}
