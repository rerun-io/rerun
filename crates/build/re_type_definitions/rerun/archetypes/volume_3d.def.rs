// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A dense 3D scalar field, rendered by ray marching.
///
/// This archetype is intended for volumetric scans and simulations, e.g. CT/MRI scans,
/// signed distance fields, or occupancy probabilities sampled on a regular grid.
///
/// The values are a 3D tensor with dimensions ordered `[z, y, x]`, i.e. the last dimension varies
/// fastest and runs along the local X axis. This matches the row-major layout of
/// [`rerun::archetypes::Image`], with slices stacked along the local Z axis.
/// The tensor element at `[k, j, i]` is thus the voxel with grid index `[i, j, k]`.
///
/// Voxels are positioned exactly like those of [`rerun::archetypes::VoxelGridMap`]:
/// the minimum corner of the voxel with `[0, 0, 0]` index is located at the origin of the entity's
/// coordinate frame and can have an additional offset from there through the optional translation
/// and rotation fields, and a voxel center is at `(index + 0.5) * voxel_size` in local grid
/// coordinates (i.e. relative to the minimum corner).
/// This archetype and a [`rerun::archetypes::VoxelGridMap`] with the same `voxel_size` and pose
/// therefore agree voxel for voxel, the dense volume covering indices `[0, 0, 0]` up to
/// `[width - 1, height - 1, depth - 1]`.
///
/// **WebGL limitation:** The viewer is not able to clip volumes against opaque scene geometry.
/// Parts of a volume can therefore appear in front of objects that should hide them.
///
/// \example archetypes/volume3d_simple title="Simple volume"
#[rerun::rerun_type]
#[docs(category = "Spatial 3D")]
#[docs(view_types = "Spatial3DView")]
#[rerun(state = "unstable")]
#[rerun(visualizer = "Volume3D")]
#[rust(derive(PartialEq))]
pub struct Volume3D {
    /// The scalar value of each voxel, as a 3D tensor with dimensions ordered `[z, y, x]`.
    ///
    /// Currently only `f16` are supported.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub values: rerun::components::TensorData,

    /// The scene-unit dimensions of a single voxel cell.
    ///
    /// This defines the voxel size along the local grid X/Y/Z axes, and thus the total extent of the
    /// volume: `[width, height, depth] * voxel_size`.
    /// Anisotropic spacing (as is common for medical scans) is expressed here.
    /// Each dimension must be finite and positive.
    ///
    /// Defaults to `[1.0, 1.0, 1.0]`.
    #[rerun(no_ui_edit)]
    #[rerun(recommended)]
    pub voxel_size: Option<rerun::components::VoxelSize>,

    /// Translation of the minimum corner of voxel `[0, 0, 0]`.
    ///
    /// Together with [`rerun::components::RotationQuat`], this defines the pose of the volume
    /// relative to the entity's coordinate frame.
    ///
    /// If not set, the minimum corner is placed at the origin of the entity's coordinate frame.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub translation: Option<rerun::components::Translation3D>,

    /// Rotation of the volume via quaternion.
    ///
    /// Together with [`rerun::components::Translation3D`], this defines the pose of the volume
    /// relative to the entity's coordinate frame.
    /// The rotation is around the minimum corner of voxel `[0, 0, 0]`, and is applied before the
    /// translation.
    #[rerun(no_ui_edit)]
    #[rerun(optional)]
    pub quaternion: Option<rerun::components::RotationQuat>,

    /// The inclusive range of voxel values to render.
    ///
    /// Values outside the range are ignored.
    ///
    /// If not specified, the range is automatically estimated from the data.
    #[rerun(optional)]
    pub value_range: Option<rerun::components::ValueRange>,

    /// Gamma correction applied to normalized voxel values before colormapping and opacity calculation.
    ///
    /// The corrected density is `normalized_value ^ gamma`.
    /// Must be finite and positive.
    /// Defaults to 3.0, suppressing low-density material and emphasizing dense structures.
    #[rerun(optional)]
    pub gamma: Option<rerun::components::GammaCorrection>,

    /// Colormap applied to the values after mapping them through `value_range` and gamma correction.
    ///
    /// Defaults to Turbo.
    #[rerun(optional)]
    pub colormap: Option<rerun::components::Colormap>,

    /// Dimensionless optical depth for the volume.
    ///
    /// For a uniform volume at the upper bound of `value_range`, this is the optical depth across one full volume-local axis.
    /// Zero is transparent, one is about 63% opaque, and larger values are denser.
    ///
    /// Defaults to 1.0.
    #[rerun(optional)]
    pub optical_density: Option<rerun::components::OpticalDensity>,
}
