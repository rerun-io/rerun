// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A 2D grid map stored as raster data in an image buffer, with a cell size in scene units and pose.
///
/// This archetype is intended for robotics applications like occupancy maps or navigation costmaps.
///
/// \example archetypes/grid_map_simple title="Simple occupancy grid map"
/// \example archetypes/grid_map_pose missing="cpp,rs" title="Log a grid map at a specific pose" image="https://static.rerun.io/grid_map_pose/55eeb468043da65a1c678f97048dca8545806983/1200w.png"
#[rerun::rerun_type]
#[cpp(no_field_ctors)]
#[docs(category = "Spatial 3D")]
#[docs(view_types = "Spatial3DView, Spatial2DView")]
#[rerun(state = "stable")]
#[rust(derive(PartialEq))]
pub struct GridMap {
    /// The raw grid data.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub data: rerun::components::ImageBuffer,

    /// The format of the grid's image data.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub format: rerun::components::ImageFormat,

    /// The scene unit size of a single grid cell (e.g. m / pixel).
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub cell_size: rerun::components::CellSize,

    /// Translation of the lower-left corner of the grid map in space.
    ///
    /// Together with [components.RotationAxisAngle] or [components.RotationQuat], this defines the pose of the
    /// lower-left image corner relative to the map's parent coordinate frame.
    ///
    /// If not set, the lower-left image corner is placed at origin of the map's parent coordinate frame.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub translation: Option<rerun::components::Translation3D>,

    /// Rotation of the lower-left corner of the grid map in space via axis + angle.
    ///
    /// Together with [components.Translation3D], this defines the pose of the
    /// lower-left image corner relative to the map's parent coordinate frame.
    ///
    /// Note: either this or [components.RotationQuat] can be set to specify the grid map's rotation, but not both.
    /// If both this and [components.RotationQuat] are set, this is ignored in favor of the quaternion.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub rotation_axis_angle: Option<rerun::components::RotationAxisAngle>,

    /// Rotation of the lower-left corner of the grid map in space via quaternion.
    ///
    /// Together with [components.Translation3D], this defines the pose of the
    /// lower-left image corner relative to the map's parent coordinate frame.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub quaternion: Option<rerun::components::RotationQuat>,

    /// Opacity of the grid map texture after all image decoding and colormap application.
    ///
    /// Defaults to 1.0 (fully opaque).
    #[rerun(optional)]
    pub opacity: Option<rerun::components::Opacity>,

    /// Optional draw order for layering multiple grid maps that overlap in space.
    ///
    /// Higher values are drawn on top of lower values.
    #[rerun(optional)]
    pub draw_order: Option<rerun::components::DrawOrder>,

    /// Colormap to use for rendering single-channel grid maps.
    ///
    /// If not set, the grid map is shown using the underlying [components.ImageFormat]
    /// interpretation.
    #[rerun(optional)]
    pub colormap: Option<rerun::components::Colormap>,
}
