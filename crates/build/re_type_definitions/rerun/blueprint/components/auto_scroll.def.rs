// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Whether the view should auto-scroll to follow the time cursor.
#[rerun::rerun_type]
#[python(aliases = "bool")]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, Default, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct AutoScroll {
    pub auto_scroll: rerun::datatypes::Bool,
}
