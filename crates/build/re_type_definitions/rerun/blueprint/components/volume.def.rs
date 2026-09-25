// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Playback volume, as a multiplier applied to the audio samples.
///
/// 0.0 is silent and 1.0 leaves the samples unchanged.
/// The scale is linear in amplitude, not in perceived loudness.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "npt.ArrayLike")]
#[docs(unreleased)]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, PartialOrd))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct Volume {
    pub volume: rerun::encodings::Float32,
}
