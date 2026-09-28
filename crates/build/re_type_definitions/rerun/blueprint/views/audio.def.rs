// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A view that shows the waveform of an [`rerun::archetypes::AssetAudio`] and plays it back.
///
/// Audio plays while time is playing on a temporal timeline, starting from the time the asset was logged.
///
/// \example views/audio title="Use a blueprint to show and play an audio asset."
#[rerun::rerun_type]
#[docs(unreleased)]
#[rerun(view_identifier = "Audio")]
#[rerun(state = "unstable")]
pub struct AudioView {
    /// Playback settings, such as volume and muting.
    pub playback: rerun::blueprint::archetypes::AudioPlayback,
}
