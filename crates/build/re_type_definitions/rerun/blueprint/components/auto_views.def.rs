// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Whether or not views should be created automatically.
#[rerun::rerun_type]
#[python(aliases = "bool")]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, Default, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct AutoViews {
    pub auto_views: rerun::datatypes::Bool,
}
