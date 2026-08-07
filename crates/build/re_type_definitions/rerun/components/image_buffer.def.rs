// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A buffer that is known to store image data.
///
/// To interpret the contents of this buffer, see, [components.ImageFormat].
#[rerun::rerun_type]
#[python(aliases = "bytes | npt.NDArray[np.uint8]")]
#[python(array_aliases = "bytes | npt.NDArray[np.uint8]")]
#[rust(derive(PartialEq, Eq))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct ImageBuffer {
    pub buffer: rerun::datatypes::Blob,
}
