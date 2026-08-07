// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Whether a [components.VideoSample] contains a keyframe (also known as a sync sample or IDR).
///
/// A keyframe in this sense must be _decoder re-entrant_: a decoder must be able to start
/// decoding the stream from this sample alone, with no prior decoder state.
/// Not every intra-coded frame qualifies. Some codecs have intra-only frames that may
/// still reference existing decoder state and are therefore not valid sync points.
/// See [components.VideoCodec] for the codec-specific definition of a keyframe.
#[rerun::rerun_type]
#[python(aliases = "bool")]
#[python(array_aliases = "bool | npt.NDArray[np.bool_]")]
#[rust(derive(Copy, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct IsKeyframe {
    pub is_keyframe: rerun::datatypes::Bool,
}
