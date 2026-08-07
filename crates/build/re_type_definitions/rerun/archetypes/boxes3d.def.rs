// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// 3D boxes with half-extents and optional center, rotations, colors etc.
///
/// If there's more instance poses than half sizes, the last box's orientation will be repeated for the remaining poses.
/// Orienting and placing boxes forms a separate transform that is applied prior to [archetypes.InstancePoses3D] and [archetypes.Transform3D].
///
/// \example archetypes/boxes3d_simple !api title="Simple 3D boxes" image="https://static.rerun.io/box3d_simple/d6a3f38d2e3360fbacac52bb43e44762635be9c8/1200w.png"
/// \example archetypes/boxes3d_batch title="Batch of 3D boxes" image="https://static.rerun.io/box3d_batch/5aac5b5d29c9f2ecd572c93f6970fcec17f4984b/1200w.png"
#[rerun::rerun_type]
#[cpp(no_field_ctors)]
#[docs(category = "Spatial 3D")]
#[docs(view_types = "Spatial3DView, Spatial2DView: if logged above active projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "Boxes3D")]
#[rust(derive(PartialEq))]
#[rust(new_pub_crate)]
pub struct Boxes3D {
    /// All half-extents that make up the batch of boxes.
    #[rerun(required)]
    pub half_sizes: Vec<rerun::components::HalfSize3D>,

    /// Optional center positions of the boxes.
    ///
    /// If not specified, the centers will be at (0, 0, 0).
    #[rerun(recommended)]
    pub centers: Option<Vec<rerun::components::Translation3D>>,

    /// Rotations via axis + angle.
    ///
    /// If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
    #[rerun(optional)]
    pub rotation_axis_angles: Option<Vec<rerun::components::RotationAxisAngle>>,

    /// Rotations via quaternion.
    ///
    /// If no rotation is specified, the axes of the boxes align with the axes of the local coordinate system.
    #[rerun(optional)]
    pub quaternions: Option<Vec<rerun::components::RotationQuat>>,

    /// Optional colors for the boxes.
    ///
    /// Alpha channel is used for transparency for solid fill-mode.
    #[rerun(recommended)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// Optional radii for the lines that make up the boxes.
    #[rerun(optional)]
    pub radii: Option<Vec<rerun::components::Radius>>,

    /// Optionally choose whether the boxes are drawn with lines or solid.
    #[rerun(optional)]
    pub fill_mode: Option<rerun::components::FillMode>,

    /// Optional text labels for the boxes.
    ///
    /// If there's a single label present, it will be placed at the center of the entity.
    /// Otherwise, each instance will have its own label.
    #[rerun(optional)]
    pub labels: Option<Vec<rerun::components::Text>>,

    /// Whether the text labels should be shown.
    ///
    /// If not set, labels will automatically appear when there is exactly one label for this entity
    /// or the number of instances on this entity is under a certain threshold.
    #[rerun(optional)]
    pub show_labels: Option<rerun::components::ShowLabels>,

    /// Optional [components.ClassId]s for the boxes.
    ///
    /// The [components.ClassId] provides colors and labels if not specified explicitly.
    #[rerun(optional)]
    pub class_ids: Option<Vec<rerun::components::ClassId>>,
}
