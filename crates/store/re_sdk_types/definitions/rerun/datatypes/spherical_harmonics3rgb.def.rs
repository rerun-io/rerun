// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Spherical harmonics coefficients of degrees 1 through 3 for RGB, as 15 half-precision RGB triples.
///
/// The coefficients are stored coefficient-major: `[c1.rgb, c2.rgb, …, c15.rgb]`.
/// The 15 coefficients `c1…c15` are ordered by ascending degree `l = 1, 2, 3`, and within
/// each degree by ascending order `m = -l … +l`:
/// degree 1 is `c1…c3`, degree 2 is `c4…c8`, and degree 3 is `c9…c15`.
/// The degrees are not exposed as separate groups, since each degree has a different
/// number of coefficients (3, 5, and 7).
///
/// This per-coefficient order matches the `f_rest_*` properties of the PLY files used by
/// [3D Gaussian Splatting](https://repo-sam.inria.fr/fungraph/3d-gaussian-splatting/) (Kerbl et al., 2023),
/// but those store the channels channel-major (all 15 coefficients of R, then G, then B),
/// so they must be transposed on import.
///
/// The degree-0 (DC) term is *not* included — it is represented as a [datatypes.Rgba32] color instead.
///
/// Data of a lower spherical harmonics degree should be zero-padded,
/// which represents the exact same function (the spherical harmonics basis is orthonormal).
/// Conversely, truncating trailing coefficients at the degree boundaries (3 and 8 triples)
/// yields the optimal least-squares approximation of that lower degree.
#[rerun::rerun_type]
#[arrow(transparent)]
#[docs(unreleased)]
#[python(aliases = "npt.ArrayLike")]
#[python(array_aliases = "npt.ArrayLike")]
#[rerun(state = "unstable")]
#[rust(derive(Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
pub struct SphericalHarmonics3Rgb {
    /// Spherical harmonics coefficients of degrees 1 through 3, coefficient-major.
    pub coefficients: [[rerun::f16; 3]; 15],
}
