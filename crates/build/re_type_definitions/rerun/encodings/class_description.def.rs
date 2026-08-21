// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The description of a semantic Class.
///
/// If an entity is annotated with a corresponding [`rerun::components::ClassId`], Rerun will use
/// the attached [`rerun::encodings::AnnotationInfo`] to derive labels and colors.
///
/// Keypoints within an annotation class can similarly be annotated with a
/// [`rerun::components::KeypointId`] in which case we should defer to the label and color for the
/// [`rerun::encodings::AnnotationInfo`] specifically associated with the Keypoint.
///
/// Keypoints within the class can also be decorated with skeletal edges.
/// Keypoint-connections are pairs of [`rerun::components::KeypointId`]s. If an edge is
/// defined, and both keypoints exist within the instance of the class, then the
/// keypoints should be connected with an edge. The edge should be labeled and
/// colored as described by the class's [`rerun::encodings::AnnotationInfo`].
///
/// \py Note that a `ClassDescription` can be directly logged using `rerun.log`.
/// \py This is equivalent to logging a `rerun.AnnotationContext` containing
/// \py a single `ClassDescription`.
#[rerun::rerun_type]
#[python(aliases = "encodings.AnnotationInfoLike")]
#[rust(arrow_opt)]
#[rust(derive(Default, Eq, PartialEq))]
#[rerun(state = "stable")]
pub struct ClassDescription {
    /// The [`rerun::encodings::AnnotationInfo`] for the class.
    pub info: rerun::encodings::AnnotationInfo,

    /// The [`rerun::encodings::AnnotationInfo`] for all of the keypoints.
    // TODO(jleibs) this could be nullable rather than forcing an empty list
    // don't null for now so we match the legacy schema
    pub keypoint_annotations: Vec<rerun::encodings::AnnotationInfo>,

    /// The connections between keypoints.
    // TODO(jleibs) this could be nullable rather than forcing an empty list
    // don't null for now so we match the legacy schema
    pub keypoint_connections: Vec<rerun::encodings::KeypointPair>,
}
