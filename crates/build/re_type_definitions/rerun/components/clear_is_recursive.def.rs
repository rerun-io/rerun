// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configures how a clear operation should behave - recursive or not.
#[rerun::rerun_type]
#[python(aliases = "bool")]
#[python(array_aliases = "bool | npt.NDArray[np.bool_]")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rust(override_crate = "re_types_core")]
#[rerun(state = "stable")]
pub struct ClearIsRecursive {
    /// If true, also clears all recursive children entities.
    pub recursive: rerun::datatypes::Bool,
}
