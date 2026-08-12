// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The order of component columns (which remain always grouped by entity path) in the dataframe view.
///
/// Entities not in this list are appended at the end in their default order.
/// Entities in this list that are not present in the view are ignored.
#[rerun::rerun_type]
#[python(aliases = "Sequence[datatypes.EntityPathLike]")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq))]
#[rerun(state = "unstable")]
pub struct ColumnOrder {
    pub entity_paths: Vec<rerun::datatypes::EntityPath>,
}
