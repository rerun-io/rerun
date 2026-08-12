// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// An orientation convention for three-dimensional coordinates.
///
/// The three directions are always ordered as [x, y, z] and specify where each positive axis points.
/// For example, [Right, Down, Forward] means that +X points right, +Y points down, and +Z points forward.
///
/// ⚠ [Rerun does not yet support left-handed coordinate systems](https://github.com/rerun-io/rerun/issues/5032).
///
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "Sequence[datatypes.ViewDirLike] | npt.ArrayLike")]
#[python(array_aliases = "ViewCoordinatesLike | npt.ArrayLike")]
#[rerun(state = "unstable")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
pub struct ViewCoordinates {
    /// The directions of the [x, y, z] axes.
    pub coordinates: [rerun::datatypes::ViewDir; 3],
}
