// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A visual representation of a [archetypes.Transform3D].
///
/// \example archetypes/transform3d_axes title="Visual representation of a transform as three arrows" image="https://static.rerun.io/transform3d_axes/574c482088e9d317b19127fc8bef957dbfd3abe8/1200w.png"
#[rerun::rerun_type]
#[docs(category = "Transforms")]
#[docs(view_types = "Spatial3DView")]
#[rerun(state = "stable")]
#[rerun(visualizer = "TransformAxes3D")]
#[rust(derive(PartialEq))]
pub struct TransformAxes3D {
    /// Visual length of the 3 axes.
    ///
    /// The length is interpreted in the local coordinate system of the transform.
    /// If the transform is scaled, the axes will be scaled accordingly.
    #[rerun(required)]
    pub axis_length: rerun::components::AxisLength,

    /// Whether to show a text label with the corresponding frame.
    #[rerun(optional)]
    pub show_frame: Option<rerun::components::ShowLabels>,
}
