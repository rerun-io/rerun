// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A zoom level determines how much of the world is visible on a map.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "npt.ArrayLike")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct ZoomLevel {
    /// Zoom level: 0 being the lowest zoom level (fully zoomed out) and 22 being the highest (fully zoomed in).
    pub zoom: rerun::datatypes::Float64,
}
