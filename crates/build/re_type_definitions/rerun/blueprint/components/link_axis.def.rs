// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Controls how the horizontal time axis is linked across time series and state timeline views.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, Default, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub enum LinkAxis {
    /// The axis is independent from all other views.
    #[default]
    Independent = 1,

    /// Link to all other views that also have this option set.
    LinkToGlobal = 2,
}
