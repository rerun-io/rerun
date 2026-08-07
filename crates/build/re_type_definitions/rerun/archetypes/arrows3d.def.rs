// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// 3D arrows with optional colors, radii, labels, etc.
///
/// \example archetypes/arrows3d_simple title="Simple batch of 3D arrows" image="https://static.rerun.io/arrow3d_simple/55e2f794a520bbf7527d7b828b0264732146c5d0/1200w.png"
#[rerun::rerun_type]
#[cpp(no_field_ctors)]
#[docs(category = "Spatial 3D")]
#[docs(view_types = "Spatial3DView, Spatial2DView: if logged above active projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "Arrows3D")]
#[rust(derive(PartialEq))]
#[rust(new_pub_crate)]
pub struct Arrows3D {
    /// All the vectors for each arrow in the batch.
    #[rerun(required)]
    pub vectors: Vec<rerun::components::Vector3D>,

    /// All the origin (base) positions for each arrow in the batch.
    ///
    /// If no origins are set, (0, 0, 0) is used as the origin for each arrow.
    #[rerun(recommended)]
    pub origins: Option<Vec<rerun::components::Position3D>>,

    /// Optional radii for the arrows.
    ///
    /// The shaft is rendered as a line with `radius = 0.5 * radius`.
    /// The tip is rendered with `height = 2.0 * radius` and `radius = 1.0 * radius`.
    #[rerun(optional)]
    pub radii: Option<Vec<rerun::components::Radius>>,

    /// Optional colors for the points.
    #[rerun(optional)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// Optional text labels for the arrows.
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

    /// Optional class Ids for the points.
    ///
    /// The [components.ClassId] provides colors and labels if not specified explicitly.
    #[rerun(optional)]
    pub class_ids: Option<Vec<rerun::components::ClassId>>,
}
