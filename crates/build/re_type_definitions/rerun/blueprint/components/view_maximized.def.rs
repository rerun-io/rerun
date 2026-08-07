// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Whether a view is maximized.
#[rerun::rerun_type]
#[python(aliases = "npt.NDArray[np.uint8] | npt.ArrayLike | Sequence[int] | bytes")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct ViewMaximized {
    pub view_id: rerun::datatypes::Uuid,
}
