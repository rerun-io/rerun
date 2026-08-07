// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// If playing, whether and how the playback time should loop.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub enum LoopMode {
    /// Looping is off.
    #[default]
    Off = 1,

    /// We are looping within the current loop selection.
    Selection = 2,

    /// We are looping the entire recording.
    ///
    /// The loop selection is ignored.
    All = 3,
}
