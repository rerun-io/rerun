// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Controls the visual bounds of a 2D view.
///
/// Everything within these bounds are guaranteed to be visible.
/// Somethings outside of these bounds may also be visible due to letterboxing.
///
/// If no visual bounds are set, it will be determined automatically,
/// based on the bounding-box of the data or other camera information present in the view.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct VisualBounds2D {
    /// Controls the visible range of a 2D view.
    ///
    /// Use this to control pan & zoom of the view.
    #[rerun(required)]
    pub range: rerun::blueprint::components::VisualBounds2D,
}
