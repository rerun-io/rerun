// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 16-bit ID representing an MCAP channel.
///
/// Used to identify specific channels within an MCAP file.
#[rerun::rerun_type]
#[python(aliases = "int")]
#[python(
    array_aliases = "int | npt.NDArray[np.uint8] | npt.NDArray[np.uint16] | npt.NDArray[np.uint32] | npt.NDArray[np.uint64]"
)]
#[rust(derive(Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct ChannelId {
    pub id: rerun::datatypes::UInt16,
}
