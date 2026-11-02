// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Overrides the component a table column's values are shown as.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq, Hash))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct ColumnType {
    pub column_type: rerun::blueprint::encodings::ColumnType,
}
