// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A helper type for mapping [datatypes.ClassId]s to class descriptions.
///
/// This is internal to [components.AnnotationContext].
#[rerun::rerun_type]
#[python(aliases = "datatypes.ClassDescriptionLike")]
#[rerun(state = "unstable")]
#[rust(derive(Default, Eq, PartialEq))]
pub struct ClassDescriptionMapElem {
    /// The key: the [components.ClassId].
    pub class_id: rerun::datatypes::ClassId,

    /// The value: class name, color, etc.
    pub class_description: rerun::datatypes::ClassDescription,
}
