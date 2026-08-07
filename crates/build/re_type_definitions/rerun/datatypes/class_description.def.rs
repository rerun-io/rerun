// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The description of a semantic Class.
///
/// If an entity is annotated with a corresponding [components.ClassId], Rerun will use
/// the attached [datatypes.AnnotationInfo] to derive labels and colors.
///
/// Keypoints within an annotation class can similarly be annotated with a
/// [components.KeypointId] in which case we should defer to the label and color for the
/// [datatypes.AnnotationInfo] specifically associated with the Keypoint.
///
/// Keypoints within the class can also be decorated with skeletal edges.
/// Keypoint-connections are pairs of [components.KeypointId]s. If an edge is
/// defined, and both keypoints exist within the instance of the class, then the
/// keypoints should be connected with an edge. The edge should be labeled and
/// colored as described by the class's [datatypes.AnnotationInfo].
///
/// \py Note that a `ClassDescription` can be directly logged using `rerun.log`.
/// \py This is equivalent to logging a `rerun.AnnotationContext` containing
/// \py a single `ClassDescription`.
#[rerun::rerun_type]
#[python(aliases = "datatypes.AnnotationInfoLike")]
#[rust(derive(Default, Eq, PartialEq))]
#[rerun(state = "stable")]
pub struct ClassDescription {
    /// The [datatypes.AnnotationInfo] for the class.
    pub info: rerun::datatypes::AnnotationInfo,

    /// The [datatypes.AnnotationInfo] for all of the keypoints.
    // TODO(jleibs) this could be nullable rather than forcing an empty list
    // don't null for now so we match the legacy schema
    pub keypoint_annotations: Vec<rerun::datatypes::AnnotationInfo>,

    /// The connections between keypoints.
    // TODO(jleibs) this could be nullable rather than forcing an empty list
    // don't null for now so we match the legacy schema
    pub keypoint_connections: Vec<rerun::datatypes::KeypointPair>,
}
