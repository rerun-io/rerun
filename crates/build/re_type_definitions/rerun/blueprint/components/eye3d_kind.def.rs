// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The kind of the 3D eye to view a scene in a [views.Spatial3DView].
///
/// This is used to specify how the controls of the view react to user input (such as mouse gestures).
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rerun(state = "stable")]
pub enum Eye3DKind {
    /// First person point of view.
    ///
    /// The camera perspective as if one is seeing it through the eyes of a person as popularized by first-person games.
    /// The center of rotation is the position of the eye (the camera).
    /// Dragging the mouse on the spatial 3D view, will rotation the scene as if one is moving
    /// their head around.
    FirstPerson = 1,

    /// Orbital eye.
    ///
    /// The center of rotation is located to a center location in front of the eye (it is different from the eye
    /// location itself), as if the eye was orbiting around the scene.
    #[default]
    Orbital = 2,
}
