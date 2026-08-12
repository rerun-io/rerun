// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Tri-state for panel controls.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub enum PanelState {
    /// Completely hidden.
    Hidden = 1,

    /// Visible, but as small as possible on its shorter axis.
    Collapsed = 2,

    /// Fully expanded.
    #[default]
    Expanded = 3,
}
