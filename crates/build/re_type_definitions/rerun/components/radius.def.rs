// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The radius of something, e.g. a point.
///
/// Internally, positive values indicate scene units, whereas negative values
/// are interpreted as UI points.
///
/// UI points are independent of zooming in Views, but are sensitive to the application UI scaling.
/// at 100% UI scaling, UI points are equal to pixels
/// The Viewer's UI scaling defaults to the OS scaling which typically is 100% for full HD screens and 200% for 4k screens.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.ArrayLike")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct Radius {
    pub value: rerun::datatypes::Float32,
}
