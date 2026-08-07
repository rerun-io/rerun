// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A sparse 3D voxel grid map with grid indices and voxel dimensions.
///
/// This archetype is intended for 3D occupancy maps and other volumetric data
/// represented as a sparse grid of voxels with scene-unit dimensions along the local X/Y/Z axes.
///
/// The minimum corner of the voxel with `[0, 0, 0]` index is located at the origin of the entity's coordinate frame
/// and can have an additional offset from there through the optional translation and rotation fields.
///
/// A voxel center is at `(index + 0.5) * voxel_size` in local grid coordinates (i.e. relative to the minimum corner).
///
/// \example archetypes/voxel_grid_map_simple title="Simple sparse voxel grid map"
#[rerun::rerun_type]
#[docs(category = "Spatial 3D")]
#[docs(view_types = "Spatial3DView")]
#[rerun(state = "unstable")]
#[rerun(visualizer = "VoxelGridMap")]
#[rust(derive(PartialEq))]
pub struct VoxelGridMap {
    /// Indices of the voxels within the grid volume.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub voxel_indices: Vec<rerun::components::VoxelIndex>,

    /// The scene-unit dimensions of a single voxel cell.
    ///
    /// This defines the voxel size along the local grid X/Y/Z axes.
    /// Each dimension must be finite and positive.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub voxel_size: rerun::components::VoxelSize,

    /// Optional scalar occupancy or value data for each voxel.
    ///
    /// If explicit colors are not provided, values are mapped through `colormap` and `value_range`.
    #[rerun(optional)]
    pub values: Option<Vec<rerun::components::VoxelValue>>,

    /// Optional colors for each voxel.
    ///
    /// If set, these colors take precedence over color-mapped scalar values.
    #[rerun(optional)]
    pub colors: Option<Vec<rerun::components::Color>>,

    /// Translation of the minimum corner of voxel `[0, 0, 0]`.
    ///
    /// Together with [components.RotationAxisAngle] or [components.RotationQuat], this defines the pose of the
    /// grid relative to the map's parent coordinate frame.
    ///
    /// If not set, the minimum corner is placed at the origin of the map's parent coordinate frame.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub translation: Option<rerun::components::Translation3D>,

    /// Rotation of the grid via axis + angle.
    ///
    /// Together with [components.Translation3D], this defines the pose of the grid relative to the
    /// map's parent coordinate frame.
    ///
    /// Note: either this or [components.RotationQuat] can be set to specify the grid's rotation, but not both.
    /// If both this and [components.RotationQuat] are set, this is ignored in favor of the quaternion.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub rotation_axis_angle: Option<rerun::components::RotationAxisAngle>,

    /// Rotation of the grid via quaternion.
    ///
    /// Together with [components.Translation3D], this defines the pose of the grid relative to the
    /// map's parent coordinate frame.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub quaternion: Option<rerun::components::RotationQuat>,

    /// Opacity of the voxels after color or colormap application.
    ///
    /// Defaults to 1.0 (fully opaque).
    #[rerun(optional)]
    pub opacity: Option<rerun::components::Opacity>,

    /// Scalar value range for color-mapping.
    ///
    /// Defaults to `[0.0, 1.0]`.
    #[rerun(optional)]
    pub value_range: Option<rerun::components::ValueRange>,

    /// Colormap to use when `values` are present and explicit `colors` are not provided.
    ///
    /// Defaults to Turbo.
    #[rerun(optional)]
    pub colormap: Option<rerun::components::Colormap>,
}
