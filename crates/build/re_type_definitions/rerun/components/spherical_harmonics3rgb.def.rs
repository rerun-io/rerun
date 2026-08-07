// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// View-dependent color, expressed as spherical harmonics coefficients of degrees 1 through 3.
///
/// The view-independent (degree-0) base color is represented as a separate [components.Color].
#[rerun::rerun_type]
#[docs(unreleased)]
#[rerun(state = "unstable")]
#[rust(derive(Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
pub struct SphericalHarmonics3Rgb {
    pub coefficients: rerun::datatypes::SphericalHarmonics3Rgb,
}
