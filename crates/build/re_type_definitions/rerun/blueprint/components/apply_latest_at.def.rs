// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Whether empty cells in a dataframe should be filled with a latest-at query.
#[rerun::rerun_type]
#[python(aliases = "bool")]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, Default, PartialEq, Eq, PartialOrd, Ord))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct ApplyLatestAt {
    pub apply_latest_at: rerun::datatypes::Bool,
}
