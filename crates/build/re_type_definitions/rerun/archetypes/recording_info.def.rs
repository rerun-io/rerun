// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A list of properties associated with a recording.
///
/// \example concepts/recording_properties !api title="Setting recording properties"
#[rerun::rerun_type]
#[rerun(state = "stable")]
#[rerun(visualizer_none)]
pub struct RecordingInfo {
    /// When the recording started.
    ///
    /// Should be an absolute time, i.e. relative to Unix Epoch.
    #[rerun(optional)]
    pub start_time: Option<rerun::components::Timestamp>,

    /// A user-chosen name for the recording.
    #[rerun(optional)]
    pub name: Option<rerun::components::Name>,
}
