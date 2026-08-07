// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// An orientation convention for a camera or 3D view.
///
/// On [archetypes.Pinhole], this component controls the camera orientation and projection direction.
/// On [SpatialInformation](https://rerun.io/docs/reference/types/views/spatial3d_view), it controls the 3D view's eye orientation, navigation, and default grid plane.
/// A logged [archetypes.ViewCoordinates] provides the default for [SpatialInformation](https://rerun.io/docs/reference/types/views/spatial3d_view).
///
/// The three directions are always ordered as [x, y, z] and specify where each positive axis points.
/// For example, [Right, Down, Forward] means that +X points right, +Y points down, and +Z points forward.
///
/// ⚠ [Rerun does not yet support left-handed coordinate systems](https://github.com/rerun-io/rerun/issues/5032).
#[rerun::rerun_type]
#[cpp(no_field_ctors)]
#[python(aliases = "npt.ArrayLike")]
#[python(array_aliases = "npt.ArrayLike")]
#[rerun(state = "unstable")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rust(repr = "transparent")]
pub struct ViewCoordinates {
    /// The directions of the [x, y, z] axes.
    pub coordinates: rerun::datatypes::ViewCoordinates,
}
