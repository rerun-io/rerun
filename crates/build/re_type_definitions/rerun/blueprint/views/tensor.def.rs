// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A view on a tensor of any dimensionality.
///
/// \example views/tensor title="Use a blueprint to create a TensorView." image="https://static.rerun.io/tensor_view/04158807b970c16af7922698389b239b0575c436/1200w.png"
#[rerun::rerun_type]
#[rerun(view_identifier = "Tensor")]
#[rerun(state = "unstable")]
pub struct TensorView {
    /// How to select the slice of the tensor to show.
    pub slice_selection: rerun::blueprint::archetypes::TensorSliceSelection,

    /// Configures how scalars are mapped to color.
    pub scalar_mapping: rerun::blueprint::archetypes::TensorScalarMapping,

    /// Configures how the selected slice should fit into the view.
    pub view_fit: rerun::blueprint::archetypes::TensorViewFit,
}
