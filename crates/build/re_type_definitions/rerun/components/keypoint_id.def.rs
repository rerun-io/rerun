// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 16-bit ID representing a type of semantic keypoint within a class.
///
/// \py `KeypointId`s are only meaningful within the context of a [`rerun::encodings::ClassDescription`].
/// \py
/// \py Used to look up an [`rerun::encodings::AnnotationInfo`] for a Keypoint within the
/// \py [`rerun::components::AnnotationContext`].
///
/// \rs `KeypointId`s are only meaningful within the context of a [`rerun::encodings::ClassDescription`].
/// \rs
/// \rs Used to look up an [`rerun::encodings::AnnotationInfo`] for a Keypoint within the [`rerun::components::AnnotationContext`].
#[rerun::rerun_type]
#[python(aliases = "int")]
#[python(
    array_aliases = "int | npt.NDArray[np.uint8] | npt.NDArray[np.uint16] | npt.NDArray[np.uint32] | npt.NDArray[np.uint64]"
)]
#[rerun(state = "stable")]
#[rust(derive(
    Default,
    Copy,
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
pub struct KeypointId {
    pub id: rerun::encodings::KeypointId,
}
