re_string_interner::declare_new_type_nonempty!(
    /// The name of a timeline. Often something like `"log_time"` or `"frame_nr"`.
    ///
    /// This name is used both as an identifier and as a display label: it is what timelines
    /// are keyed on everywhere (`TimePoint`, the chunk store, the manifest index, …), and it
    /// is also what the user reads in the time panel. There is no separate display name.
    ///
    /// Being the key has consequences: two timelines with the same name are the same
    /// timeline, and a `Timeline`'s `TimeType` is payload, not part of its identity.
    ///
    /// Using the same [`TimelineName`] with different `TimeType`s (or changing its type) is undefined behavior.
    pub struct TimelineName;
);

impl TimelineName {
    /// The log time timeline to which all API functions will always log.
    ///
    /// This timeline is automatically maintained by the SDKs and captures the wall-clock time at
    /// which point the data was logged (according to the client's wall-clock).
    #[inline]
    pub fn log_time() -> Self {
        re_string_interner::intern_static_nonempty!(TimelineName, "log_time")
    }

    /// The log tick timeline to which all API functions will always log.
    ///
    /// This timeline is automatically maintained by the SDKs and captures the logging tick at
    /// which point the data was logged.
    /// The logging tick is monotically incremented each time the client calls one of the logging
    /// methods on a `RecordingStream`.
    #[inline]
    pub fn log_tick() -> Self {
        re_string_interner::intern_static_nonempty!(TimelineName, "log_tick")
    }
}
