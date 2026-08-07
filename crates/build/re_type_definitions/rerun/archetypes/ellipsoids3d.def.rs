// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// 3D ellipsoids or spheres.
///
/// This archetype is for ellipsoids or spheres whose size is a key part of the data
/// (e.g. a bounding sphere).
/// For points whose radii are for the sake of visualization, use [archetypes.Points3D] instead.
///
/// If there's more instance poses than half sizes, the last ellipsoid/sphere's orientation will be repeated for the remaining poses.
/// Orienting and placing ellipsoids/spheres forms a separate transform that is applied prior to [archetypes.InstancePoses3D] and [archetypes.Transform3D].
///
/// \example archetypes/ellipsoids3d_simple title="Covariance ellipsoid" image="https://static.rerun.io/elliopsoid3d_simple/bd5d46e61b80ae44792b52ee07d750a7137002ea/1200w.png"
#[rerun::rerun_type]
#[cpp(no_field_ctors)]
#[docs(category = "Spatial 3D")]
#[docs(view_types = "Spatial3DView, Spatial2DView: if logged above active projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "Ellipsoids3D")]
#[rust(derive(PartialEq))]
#[rust(new_pub_crate)]
pub struct Ellipsoids3D {
    /// For each ellipsoid, half of its size on its three axes.
    ///
    /// If all components are equal, then it is a sphere with that radius.
    #[rerun(required)]
    pub half_sizes: Vec<rerun::components::HalfSize3D>,

    /// Optional center positions of the ellipsoids.
    ///
    /// If not specified, the centers will be at (0, 0, 0).
    #[rerun(recommended)]
    pub centers: Option<Vec<rerun::components::Translation3D>>,

    /// Rotations via axis + angle.
    ///
    /// If no rotation is specified, the axes of the ellipsoid align with the axes of the local coordinate system.
    #[rerun(optional)]
    pub rotation_axis_angles: Option<Vec<rerun::components::RotationAxisAngle>>,

    /// Rotations via quaternion.
    ///
    /// If no rotation is specified, the axes of the ellipsoid align with the axes of the local coordinate system.
    #[rerun(optional)]
    pub quaternions: Option<Vec<rerun::components::RotationQuat>>,

    /// Optional colors for the ellipsoids.
    ///
    /// Alpha channel is used for transparency for solid fill-mode.
    #[rerun(recommended)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// Optional radii for the lines used when the ellipsoid is rendered as a wireframe.
    #[rerun(optional)]
    pub line_radii: Option<Vec<rerun::components::Radius>>,

    /// Optionally choose whether the ellipsoids are drawn with lines or solid.
    #[rerun(optional)]
    pub fill_mode: Option<rerun::components::FillMode>,

    /// Optional text labels for the ellipsoids.
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
