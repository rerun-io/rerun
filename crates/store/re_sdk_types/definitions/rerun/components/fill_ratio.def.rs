// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// How much a primitive fills out the available space.
///
/// Used for instance to scale the points of the point cloud created from [archetypes.DepthImage] projection in 3D views.
/// Valid range is from 0 to max float although typically values above 1.0 are not useful.
///
/// Defaults to 1.0.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.ArrayLike")]
#[rust(derive(Copy, PartialEq, PartialOrd, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct FillRatio {
    pub value: rerun::datatypes::Float32,
}
