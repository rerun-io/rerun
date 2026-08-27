// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Determines which faces of a mesh are rendered.
///
/// For this purpose, we assume that the winding order of vertices in a mesh is
/// consistent and that front faces are defined as those with vertices in counter clockwise order.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
pub enum MeshFaceRendering {
    /// Show both back and front faces.
    #[default]
    DoubleSided = 1,

    /// Only front faces are shown.
    ///
    /// Front faces are assumed to have a counter clockwise vertex winding order on screen.
    Front = 2,

    /// Only back faces are shown.
    ///
    /// Back faces are assumed to have a clockwise vertex winding order on screen.
    Back = 3,
}
