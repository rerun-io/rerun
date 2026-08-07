// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The annotation context provides additional information on how to display entities.
///
/// Entities can use [datatypes.ClassId]s and [datatypes.KeypointId]s to provide annotations, and
/// the labels and colors will be looked up in the appropriate
/// annotation context. We use the *first* annotation context we find in the
/// path-hierarchy when searching up through the ancestors of a given entity
/// path.
#[rerun::rerun_type]
#[python(
    aliases = "datatypes.ClassDescriptionArrayLike | Sequence[datatypes.ClassDescriptionMapElemLike]"
)]
#[rerun(state = "unstable")]
#[rust(derive(Default, Eq, PartialEq))]
pub struct AnnotationContext {
    /// List of class descriptions, mapping class indices to class names, colors etc.
    pub class_map: Vec<rerun::datatypes::ClassDescriptionMapElem>,
}
