// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configures how a selected tensor slice is shown on screen.
#[rerun::rerun_type]
#[python(aliases = "blueprint_components.ViewFitLike")]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct TensorViewFit {
    /// How the image is scaled to fit the view.
    #[rerun(optional)]
    pub scaling: Option<rerun::blueprint::components::ViewFit>,
}
