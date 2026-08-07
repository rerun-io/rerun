// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A line strip in 2D space.
///
/// A line strip is a list of points connected by line segments. It can be used to draw
/// approximations of smooth curves.
///
/// The points will be connected in order, like so:
/// ```text
///        2------3     5
///       /        \   /
/// 0----1          \ /
///                  4
/// ```
#[rerun::rerun_type]
#[python(aliases = "datatypes.Vec2DArrayLike | npt.NDArray[np.float32]")]
#[python(array_aliases = "npt.NDArray[np.float32]")]
#[rust(derive(Default, PartialEq))]
#[rerun(state = "stable")]
pub struct LineStrip2D {
    pub points: Vec<rerun::datatypes::Vec2D>,
}
