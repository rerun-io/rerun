// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The distance from the camera origin to the image plane when the projection is shown in a 3D viewer.
///
/// This is only used for visualization purposes, and does not affect the projection itself.
#[rerun::rerun_type]
#[rust(derive(Copy, PartialEq, PartialOrd))]
#[rerun(state = "stable")]
pub struct ImagePlaneDistance {
    pub image_from_camera: rerun::datatypes::Float32,
}
