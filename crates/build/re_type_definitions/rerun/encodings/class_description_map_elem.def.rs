// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A helper type for mapping [`rerun::encodings::ClassId`]s to class descriptions.
///
/// This is internal to [`rerun::components::AnnotationContext`].
#[rerun::rerun_type]
#[python(aliases = "encodings.ClassDescriptionLike")]
#[rerun(state = "unstable")]
#[rust(arrow_opt)]
#[rust(derive(Default, Eq, PartialEq))]
pub struct ClassDescriptionMapElem {
    /// The key: the [`rerun::components::ClassId`].
    pub class_id: rerun::encodings::ClassId,

    /// The value: class name, color, etc.
    pub class_description: rerun::encodings::ClassDescription,
}
