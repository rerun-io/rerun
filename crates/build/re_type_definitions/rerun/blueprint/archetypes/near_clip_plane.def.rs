// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Controls the distance to the near clip plane in 3D scene units.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct NearClipPlane {
    /// Controls the distance to the near clip plane in 3D scene units.
    ///
    /// Content closer than this distance will not be visible.
    #[rerun(optional)]
    pub near_clip_plane: rerun::blueprint::components::NearClipPlane,
}
