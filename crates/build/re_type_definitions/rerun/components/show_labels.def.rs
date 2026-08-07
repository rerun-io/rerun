// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Whether the entity's [components.Text] label is shown.
///
/// The main purpose of this component existing separately from the labels themselves
/// is to be overridden when desired, to allow hiding and showing from the viewer and
/// blueprints.
#[rerun::rerun_type]
#[python(aliases = "bool")]
#[python(array_aliases = "bool | npt.NDArray[np.bool_]")]
#[rust(derive(Copy, PartialEq, Eq))]
#[rerun(state = "stable")]
pub struct ShowLabels {
    /// Whether the entity's [components.Text] label is shown.
    pub show_labels: rerun::datatypes::Bool,
}
