// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The active tab in a tabbed container.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct ActiveTab {
    /// Which tab is currently active.
    ///
    /// This should always correspond to a tab in the container.
    pub tab: rerun::datatypes::EntityPath,
}
