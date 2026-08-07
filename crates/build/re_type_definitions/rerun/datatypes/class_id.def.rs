// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 16-bit ID representing a type of semantic class.
///
/// \rs Used to look up a [`crate::datatypes::ClassDescription`] within the [`crate::components::AnnotationContext`].
#[rerun::rerun_type]
#[arrow(transparent)]
#[python(aliases = "int")]
#[python(array_aliases = "int | npt.ArrayLike")]
#[rust(derive(
    Copy,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    bytemuck::Pod,
    bytemuck::Zeroable,
    ::serde::Serialize,
    ::serde::Deserialize
))]
#[rust(repr = "transparent")]
#[rust(tuple_struct)]
#[rerun(state = "stable")]
pub struct ClassId {
    pub id: u16,
}
