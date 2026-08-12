// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Distance to the near clip plane used for `Spatial2DView`.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable))]
#[rust(repr = "transparent")]
#[rerun(state = "unstable")]
pub struct NearClipPlane {
    /// Distance to the near clip plane in 3D scene units.
    pub near_clip_plane: rerun::datatypes::Float32,
}
