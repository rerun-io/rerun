// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// How densely a view presents its content.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rerun(state = "unstable")]
pub enum ViewDensity {
    /// Present content with more spacing and visual structure.
    #[default]
    Spacious = 1,

    /// Present content using less space.
    Compact = 2,
}
