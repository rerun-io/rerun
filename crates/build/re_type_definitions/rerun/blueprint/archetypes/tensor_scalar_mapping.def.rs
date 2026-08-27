// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Configures how tensor scalars are mapped to color.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rust(derive(Default))]
#[rerun(state = "unstable")]
pub struct TensorScalarMapping {
    /// Filter used when zooming in on the tensor.
    ///
    /// Note that the filter is applied to the scalar values *before* they are mapped to color.
    #[rerun(optional)]
    pub mag_filter: Option<rerun::components::MagnificationFilter>,

    /// How scalar values map to colors.
    #[rerun(optional)]
    pub colormap: Option<rerun::components::Colormap>,

    /// Gamma exponent applied to normalized values before mapping to color.
    ///
    /// Raises the normalized values to the power of this value before mapping to color.
    /// Acts like an inverse brightness. Defaults to 1.0.
    ///
    /// The final value for display is set as:
    /// `colormap( ((value - data_display_range.min) / (data_display_range.max - data_display_range.min)) ** gamma )`
    #[rerun(optional)]
    pub gamma: Option<rerun::components::GammaCorrection>,
}
