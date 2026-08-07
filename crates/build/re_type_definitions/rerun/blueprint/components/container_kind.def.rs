// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The kind of a blueprint container (tabs, grid, …).
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rerun(state = "stable")]
pub enum ContainerKind {
    /// Put children in separate tabs
    Tabs = 1,

    /// Order the children left to right
    Horizontal = 2,

    /// Order the children top to bottom
    Vertical = 3,

    /// Organize children in a grid layout
    #[default]
    Grid = 4,
}
