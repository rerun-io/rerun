// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

// TODO(#2427): distinguish optional vs. recommended in language backends

/// A 2D point cloud with positions and optional colors, radii, labels, etc.
///
/// \example archetypes/points2d_simple !api title="Simple 2D points" image="https://static.rerun.io/point2d_simple/66e33b237ecd3d51363e56706566c5e7a58fe075/1200w.png"
/// \example archetypes/points2d_random title="Randomly distributed 2D points with varying color and radius" image="https://static.rerun.io/point2d_random/8e8ac75373677bd72bd3f56a15e44fcab309a168/1200w.png"
/// \example archetypes/points2d_ui_radius title="Log points with radii given in UI points" image="https://static.rerun.io/point2d_ui_radius/ce804fc77300d89c348b4ab5960395171497b7ac/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Spatial 2D")]
#[docs(view_types = "Spatial2DView, Spatial3DView: if logged under a projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "Points2D")]
#[rust(derive(PartialEq))]
pub struct Points2D {
    /// All the 2D positions at which the point cloud shows points.
    #[rerun(required)]
    pub positions: Vec<rerun::components::Position2D>,

    /// Optional radii for the points, effectively turning them into circles.
    #[rerun(recommended)]
    pub radii: Option<Vec<rerun::components::Radius>>,

    /// Optional colors for the points.
    ///
    /// \py The colors are interpreted as RGB or RGBA in sRGB gamma-space,
    /// \py As either 0-1 floats or 0-255 integers, with separate alpha.
    #[rerun(recommended)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// Optional text labels for the points.
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
    /// Defaults to `30.0`.
    #[rerun(optional)]
    pub draw_order: Option<rerun::components::DrawOrder>,

    /// Optional class Ids for the points.
    ///
    /// The [components.ClassId] provides colors and labels if not specified explicitly.
    #[rerun(optional)]
    pub class_ids: Option<Vec<rerun::components::ClassId>>,

    /// Optional keypoint IDs for the points, identifying them within a class.
    ///
    /// If keypoint IDs are passed in but no [components.ClassId]s were specified, the [components.ClassId] will
    /// default to 0.
    /// This is useful to identify points within a single classification (which is identified
    /// with `class_id`).
    /// E.g. the classification might be 'Person' and the keypoints refer to joints on a
    /// detected skeleton.
    #[rerun(optional)]
    pub keypoint_ids: Option<Vec<rerun::components::KeypointId>>,
}
