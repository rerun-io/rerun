// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Describe a component column to be selected in the dataframe view.
// TODO(ab, jleibs): this probably needs reunification with whatever structure the data out API uses.
#[rerun::rerun_type]
#[python(aliases = "str")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default, PartialEq, Eq, Hash))]
#[rerun(state = "unstable")]
pub struct ComponentColumnSelector {
    /// The entity path for this component.
    pub entity_path: rerun::datatypes::EntityPath,

    /// The name of the component.
    ///
    /// This acts as the component name in the context of a given `entity_path`
    /// An example for this would be `Points3D:positions`, for the `positions` field in [archetypes.Points3D].
    pub component: rerun::datatypes::Utf8,
    //TODO(ab, jleibs): many more fields to come (archetype, etc.)
}
