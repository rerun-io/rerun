// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A binary blob of data.
/// \rs
/// \rs Ref-counted internally and therefore cheap to clone.
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "bytes | npt.NDArray[np.uint8]")]
#[python(array_aliases = "bytes | npt.NDArray[np.uint8]")]
#[rust(derive(PartialEq))]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct Blob {
    pub data: Vec<u8>,
}
