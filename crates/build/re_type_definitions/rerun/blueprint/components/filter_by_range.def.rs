// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configuration for a filter-by-range feature of the dataframe view.
//TODO(ab, jleibs): this probably needs reunification with whatever structure the data out API uses.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(PartialEq, Eq))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct FilterByRange {
    pub range: rerun::blueprint::datatypes::FilterByRange,
}
