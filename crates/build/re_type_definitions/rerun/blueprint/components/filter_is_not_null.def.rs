// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration for the filter is not null feature of the dataframe view.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct FilterIsNotNull {
    pub filter_is_not_null: rerun::blueprint::datatypes::FilterIsNotNull,
}
