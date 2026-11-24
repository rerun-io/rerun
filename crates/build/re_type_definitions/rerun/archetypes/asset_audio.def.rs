// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// An audio file, stored as-is (`.aac`, `.flac`, `.m4a`, `.mp3`, `.ogg`, `.wav`).
///
/// The audio starts playing at the time it was logged,
/// so log it on a temporal timeline (duration or timestamp) at the point where playback should begin.
/// The viewer plays it while time is playing, in an [`rerun::blueprint::views::AudioView`],
/// which also shows the waveform.
///
/// The viewer can also open audio files directly, like any other supported file.
///
/// Streaming raw PCM samples (e.g. from a microphone) is not supported yet.
/// As a workaround, encode the samples as an in-memory WAV file and log that.
/// For live audio, encode short segments, and log each segment at the time it starts.
/// To plot the raw signal instead of playing it, log the samples as [`rerun::archetypes::Scalars`].
///
/// \example archetypes/asset_audio_simple title="Simple audio asset"
#[rerun::rerun_type]
#[docs(category = "Audio")]
#[docs(view_types = "AudioView")]
#[rerun(state = "unstable")]
#[rerun(visualizer = "AssetAudio")]
pub struct AssetAudio {
    /// The asset's bytes.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub blob: rerun::components::Blob,

    /// The Media Type of the asset.
    ///
    /// For instance:
    /// * `audio/aac` (raw ADTS stream)
    /// * `audio/flac`
    /// * `audio/mp4` (M4A)
    /// * `audio/mpeg` (MP3)
    /// * `audio/ogg`
    /// * `audio/wav`
    ///
    /// Any audio media type can be stored.
    /// Which ones the viewer can decode depends on the viewer version.
    ///
    /// If omitted, the viewer will try to guess from the data blob.
    /// If it cannot guess, it won't be able to play the asset.
    #[rerun(recommended)]
    pub media_type: Option<rerun::components::MediaType>,
}
