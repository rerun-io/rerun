// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The container that sits at the root of a viewport.
#[rerun::rerun_type]
#[python(aliases = "npt.NDArray[np.uint8] | npt.ArrayLike | Sequence[int] | bytes")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct RootContainer {
    /// `ContainerId` for the root.
    pub id: rerun::datatypes::Uuid,
}
