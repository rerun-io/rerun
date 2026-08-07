// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 16-bit unique identifier for a schema within the MCAP file.
#[rerun::rerun_type]
#[python(aliases = "int")]
#[python(
    array_aliases = "int | npt.NDArray[np.uint8] | npt.NDArray[np.uint16] | npt.NDArray[np.uint32] | npt.NDArray[np.uint64]"
)]
#[rust(derive(Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct SchemaId {
    pub id: rerun::datatypes::UInt16,
}
