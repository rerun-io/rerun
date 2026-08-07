// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The six cardinal directions for 3D view-space.
///
/// Note that these are abstract directions and do on their own not correspond to any specific
/// coordinate system or numerical values.
#[rerun::rerun_type]
#[repr(u8)]
#[docs(unreleased)]
#[rerun(state = "stable")]
pub enum ViewDir {
    /// Up.
    Up = 1,

    /// Down.
    Down = 2,

    /// Right.
    Right = 3,

    /// Left.
    Left = 4,

    /// Forward.
    Forward = 5,

    /// Back.
    Back = 6,
}
