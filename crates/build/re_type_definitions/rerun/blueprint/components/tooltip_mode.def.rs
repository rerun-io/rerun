// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Controls how the plot tooltip behaves when hovering over data.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub enum TooltipMode {
    /// Show only the nearest data point to the cursor.
    #[default]
    Nearest = 1,

    /// Show values of all visible series at the hovered time.
    All = 2,
}
