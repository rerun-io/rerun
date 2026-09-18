// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Dimensionless natural-log optical depth.
///
/// High values lead to more opaque surfaces, lower to more transparent ones.
/// More accurately, a value of 1 attenuates light to `exp(-1)` over the reference distance.
#[rerun::rerun_type]
#[docs(unreleased)]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.ArrayLike")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct OpticalDensity {
    pub optical_density: rerun::encodings::Float32,
}
