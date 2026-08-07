// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The highest spherical harmonics degree to evaluate when rendering, 0-3.
///
/// `0` renders the view-independent base color only, and is the fastest.
/// Each higher degree brings in more view-dependent detail, at the cost of fetching and
/// evaluating more coefficients ([components.SphericalHarmonics3Rgb]):
/// 3 of them for degree 1, 8 for degree 2, and all 15 for degree 3.
///
/// Lowering this in the blueprint can make the rendering a lot faster.
///
/// Defaults to 3, i.e. every coefficient the data has.
#[rerun::rerun_type]
#[docs(unreleased)]
#[python(aliases = "int")]
#[python(
    array_aliases = "int | npt.NDArray[np.uint8] | npt.NDArray[np.uint16] | npt.NDArray[np.uint32]"
)]
#[rerun(state = "unstable")]
#[rust(derive(Copy, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
pub struct SphericalHarmonicsDegree {
    pub degree: rerun::datatypes::UInt32,
}
