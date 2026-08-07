// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 16-byte UUID.
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "npt.NDArray[Any] | npt.ArrayLike | Sequence[int] | bytes")]
#[python(
    array_aliases = "npt.NDArray[Any] | npt.ArrayLike | Sequence[Sequence[int]] | Sequence[int] | Sequence[bytes]"
)]
#[rust(derive(Default, Copy, PartialEq, Eq, PartialOrd, Ord, Hash))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct Uuid {
    /// The raw bytes representing the UUID.
    pub bytes: [u8; 16],
}
