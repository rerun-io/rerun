// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Annotation info annotating a class id or key-point id.
///
/// Color and label will be used to annotate entities/keypoints which reference the id.
/// The id refers either to a class or key-point id
#[rerun::rerun_type]
#[python(aliases = "int | Tuple[int, str] | Tuple[int, str, datatypes.Rgba32Like]")]
#[rust(derive(Default, Eq, PartialEq))]
#[rerun(state = "stable")]
pub struct AnnotationInfo {
    /// [datatypes.ClassId] or [datatypes.KeypointId] to which this annotation info belongs.
    // TODO(jleibs): make this typed
    pub id: u16,

    /// The label that will be shown in the UI.
    pub label: Option<rerun::datatypes::Utf8>,

    /// The color that will be applied to the annotated entity.
    pub color: Option<rerun::datatypes::Rgba32>,
}
