// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// 3D capsules; cylinders with hemispherical caps.
///
/// Capsules are defined by two endpoints (the centers of their end cap spheres), which are located
/// at (0, 0, 0) and (0, 0, length), that is, extending along the positive direction of the Z axis.
/// Capsules in other orientations may be produced by applying a rotation to the entity or
/// instances.
///
/// If there's more instance poses than lengths & radii, the last capsule's orientation will be repeated for the remaining poses.
/// Orienting and placing capsules forms a separate transform that is applied prior to [archetypes.InstancePoses3D] and [archetypes.Transform3D].
///
/// \example archetypes/capsules3d_batch title="Batch of capsules" image="https://static.rerun.io/capsule3d_batch/6e6a4acafcf528359372147d7247f85d84434101/1200w.png"
#[rerun::rerun_type]
#[cpp(no_field_ctors)]
#[docs(category = "Spatial 3D")]
#[docs(view_types = "Spatial3DView, Spatial2DView: if logged above active projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "Capsules3D")]
#[rust(derive(PartialEq))]
#[rust(new_pub_crate)]
pub struct Capsules3D {
    /// Lengths of the capsules, defined as the distance between the centers of the endcaps.
    #[rerun(required)]
    pub lengths: Vec<rerun::components::Length>,

    /// Radii of the capsules.
    #[rerun(required)]
    pub radii: Vec<rerun::components::Radius>,

    /// Optional translations of the capsules.
    ///
    /// If not specified, one end of each capsule will be at (0, 0, 0).
    #[rerun(recommended)]
    pub translations: Option<Vec<rerun::components::Translation3D>>,

    /// Rotations via axis + angle.
    ///
    /// If no rotation is specified, the capsules align with the +Z axis of the local coordinate system.
    #[rerun(optional)]
    pub rotation_axis_angles: Option<Vec<rerun::components::RotationAxisAngle>>,

    /// Rotations via quaternion.
    ///
    /// If no rotation is specified, the capsules align with the +Z axis of the local coordinate system.
    #[rerun(optional)]
    pub quaternions: Option<Vec<rerun::components::RotationQuat>>,

    /// Optional colors for the capsules.
    ///
    /// Alpha channel is used for transparency for solid fill-mode.
    #[rerun(recommended)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// Optional radii for the lines used when the cylinder is rendered as a wireframe.
    #[rerun(optional)]
    pub line_radii: Option<Vec<rerun::components::Radius>>,

    /// Optionally choose whether the cylinders are drawn with lines or solid.
    #[rerun(optional)]
    pub fill_mode: Option<rerun::components::FillMode>,

    /// Optional text labels for the capsules, which will be located at their centers.
    #[rerun(optional)]
    pub labels: Option<Vec<rerun::components::Text>>,

    /// Whether the text labels should be shown.
    ///
    /// If not set, labels will automatically appear when there is exactly one label for this entity
    /// or the number of instances on this entity is under a certain threshold.
    #[rerun(optional)]
    pub show_labels: Option<rerun::components::ShowLabels>,

    /// Optional class ID for the ellipsoids.
    ///
    /// The class ID provides colors and labels if not specified explicitly.
    #[rerun(optional)]
    pub class_ids: Option<Vec<rerun::components::ClassId>>,
}
