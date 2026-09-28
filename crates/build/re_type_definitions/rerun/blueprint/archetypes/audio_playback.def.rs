// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Playback settings for an audio view.
#[rerun::rerun_type]
#[docs(unreleased)]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct AudioPlayback {
    /// Playback volume, from 0.0 (silent) to 1.0 (full).
    ///
    /// Defaults to 1.0.
    #[rerun(optional)]
    pub volume: Option<rerun::blueprint::components::Volume>,
}
