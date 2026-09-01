// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Controls when data point markers are displayed on line series.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub enum PointsDisplay {
    /// Show data point markers only when hovering near them.
    #[default]
    OnHover = 1,

    /// Always show data point markers on non-aggregated line series.
    Always = 2,
}
