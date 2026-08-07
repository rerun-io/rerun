// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// How should the horizontal/X/time axis be linked across multiple plots
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, Default, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub enum LinkAxis {
    /// The axis is independent from all other plots.
    #[default]
    Independent = 1,

    /// Link to all other plots that also have this options set.
    LinkToGlobal = 2,
}
