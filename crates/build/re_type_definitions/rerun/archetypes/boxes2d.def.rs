// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// 2D boxes with half-extents and optional center, colors etc.
///
/// \example archetypes/boxes2d_simple title="Simple 2D boxes" image="https://static.rerun.io/box2d_simple/ac4424f3cf747382867649610cbd749c45b2020b/1200w.png"
#[rerun::rerun_type]
#[cpp(no_field_ctors)]
#[docs(category = "Spatial 2D")]
#[docs(view_types = "Spatial2DView, Spatial3DView: if logged under a projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "Boxes2D")]
#[rust(derive(PartialEq))]
#[rust(new_pub_crate)]
pub struct Boxes2D {
    /// All half-extents that make up the batch of boxes.
    #[rerun(required)]
    pub half_sizes: Vec<rerun::components::HalfSize2D>,

    /// Optional center positions of the boxes.
    #[rerun(recommended)]
    pub centers: Option<Vec<rerun::components::Position2D>>,

    // TODO(#3247): Add 2D rotation.
    // Optional rotations of the boxes.
    //#[rerun(recommended)]
    //pub rotations: Option<Vec<rerun::components::Rotation2D>>,
    /// Optional colors for the boxes.
    #[rerun(recommended)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// Optional radii for the lines that make up the boxes.
    #[rerun(optional)]
    pub radii: Option<Vec<rerun::components::Radius>>,

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

    /// An optional floating point value that specifies the 2D drawing order.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    /// Defaults to `10.0`.
    #[rerun(optional)]
    pub draw_order: Option<rerun::components::DrawOrder>,

    /// Optional [components.ClassId]s for the boxes.
    ///
    /// The [components.ClassId] provides colors and labels if not specified explicitly.
    #[rerun(optional)]
    pub class_ids: Option<Vec<rerun::components::ClassId>>,
}
