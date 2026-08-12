// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The world->depth map scaling factor.
///
/// This measures how many depth map units are in a world unit.
/// For instance, if a depth map uses millimeters and the world uses meters,
/// this value would be `1000`.
///
/// Note that the only effect on 2D views is the physical depth values shown when hovering the image.
/// In 3D views on the other hand, this affects where the points of the point cloud are placed.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.NDArray[np.float32]")]
#[rerun(state = "unstable")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
pub struct DepthMeter {
    pub value: rerun::datatypes::Float32,
}
