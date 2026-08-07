// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// 3D cylinders with flat caps.
///
/// This archetype is for cylinder primitives defined by their axial length and radius.
/// For points whose radii are for visualization purposes, use [archetypes.Points3D] instead.
///
/// Orienting and placing cylinders forms a separate transform that is applied prior to [archetypes.InstancePoses3D] and [archetypes.Transform3D].
///
/// \example archetypes/cylinders3d_batch title="Batch of cylinders" image="https://static.rerun.io/cylinders3d_batch/ef642dede2bef23704eaff0f22aa48284d482b23/1200w.png"
#[rerun::rerun_type]
#[cpp(no_field_ctors)]
#[docs(category = "Spatial 3D")]
#[docs(view_types = "Spatial3DView, Spatial2DView: if logged above active projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "Cylinders3D")]
#[rust(derive(PartialEq))]
#[rust(new_pub_crate)]
pub struct Cylinders3D {
    /// The total axial length of the cylinder, measured as the straight-line distance between the centers of its two endcaps.
    #[rerun(required)]
    pub lengths: Vec<rerun::components::Length>,

    /// Radii of the cylinders.
    #[rerun(required)]
    pub radii: Vec<rerun::components::Radius>,

    /// Optional centers of the cylinders.
    ///
    /// If not specified, each cylinder will be centered at (0, 0, 0).
    #[rerun(recommended)]
    pub centers: Option<Vec<rerun::components::Translation3D>>,

    /// Rotations via axis + angle.
    ///
    /// If no rotation is specified, the cylinders align with the +Z axis of the local coordinate system.
    #[rerun(optional)]
    pub rotation_axis_angles: Option<Vec<rerun::components::RotationAxisAngle>>,

    /// Rotations via quaternion.
    ///
    /// If no rotation is specified, the cylinders align with the +Z axis of the local coordinate system.
    #[rerun(optional)]
    pub quaternions: Option<Vec<rerun::components::RotationQuat>>,

    /// Optional colors for the cylinders.
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

    /// Optional text labels for the cylinders, which will be located at their centers.
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
