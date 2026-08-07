// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// All the contents in the container.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct IncludedContent {
    /// List of the contents by [datatypes.EntityPath].
    ///
    /// This must be a path in the blueprint store.
    /// Typically structure as `<blueprint_registry>/<uuid>`.
    // TODO(jleibs): Maybe make this a typed UUID in the future.
    pub contents: rerun::datatypes::EntityPath,
}
