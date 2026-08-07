// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Video sample data (also known as "video chunk").
///
/// Each video sample must contain enough data for exactly one video frame
/// (this restriction may be relaxed in the future for some codecs).
///
/// Keyframes may require additional data, for details see [components.VideoCodec].
#[rerun::rerun_type]
#[python(aliases = "bytes | npt.NDArray[np.uint8]")]
#[python(array_aliases = "bytes | npt.NDArray[np.uint8]")]
#[rust(derive(PartialEq, Eq))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct VideoSample {
    pub buffer: rerun::datatypes::Blob,
}
