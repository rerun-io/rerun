// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 3D point cloud with positions and optional colors, radii, labels, etc.
///
/// If there are multiple instance poses, the entire point cloud will be repeated for each of the poses.
///
/// \example archetypes/points3d_simple title="Simple 3D points" image="https://static.rerun.io/point3d_simple/32fb3e9b65bea8bd7ffff95ad839f2f8a157a933/1200w.png"
/// \example archetypes/points3d_random !api title="Randomly distributed 3D points with varying color and radius" image="https://static.rerun.io/point3d_random/7e94e1806d2c381943748abbb3bedb68d564de24/1200w.png"
/// \example archetypes/points3d_ui_radius !api title="Log points with radii given in UI points" image="https://static.rerun.io/point3d_ui_radius/e051a65b4317438bcaea8d0eee016ac9460b5336/1200w.png"
/// \example archetypes/points3d_row_updates title="Update a point cloud over time" image="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/1200w.png"
/// \example archetypes/points3d_column_updates title="Update a point cloud over time, in a single operation" image="https://static.rerun.io/points3d_row_updates/fba056871b1ec3fc6978ab605d9a63e44ef1f6de/1200w.png"
/// \example archetypes/points3d_partial_updates title="Update specific properties of a point cloud over time" image="https://static.rerun.io/points3d_partial_updates/d8bec9c3388d2bd0fe59dff01ab8cde0bdda135e/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Spatial 3D")]
#[docs(view_types = "Spatial3DView, Spatial2DView: if logged above active projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "Points3D")]
#[rust(derive(PartialEq))]
pub struct Points3D {
    /// All the 3D positions at which the point cloud shows points.
    #[rerun(required)]
    pub positions: Vec<rerun::components::Position3D>,

    /// Optional radii for the points, effectively turning them into circles.
    #[rerun(recommended)]
    pub radii: Option<Vec<rerun::components::Radius>>,

    /// Optional colors for the points.
    ///
    /// \py The colors are interpreted as RGB or RGBA in sRGB gamma-space,
    /// \py As either 0-1 floats or 0-255 integers, with separate alpha.
    ///
    /// By default, the alpha channel affects brightness rather than transparency.
    /// TODO(#1611): To use the alpha channel for transparency, enable the experimental "Transparent point clouds" feature flag.
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

    /// How points should be shaded.
    ///
    /// If not set, points are rendered with [components.PointShading.Gradient] by default.
    #[rerun(optional)]
    pub point_shading: Option<rerun::components::PointShading>,

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
