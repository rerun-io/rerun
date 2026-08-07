// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Specifies a 2D slice of a tensor.
#[rerun::rerun_type]
#[rerun(scope = "blueprint")]
#[rerun(state = "unstable")]
pub struct TensorSliceSelection {
    /// Which dimension to map to width.
    ///
    /// If not specified, the height will be determined automatically based on the name and index of the dimension.
    #[rerun(optional)]
    pub width: Option<rerun::components::TensorWidthDimension>,

    /// Which dimension to map to height.
    ///
    /// If not specified, the height will be determined automatically based on the name and index of the dimension.
    #[rerun(optional)]
    pub height: Option<rerun::components::TensorHeightDimension>,

    /// Selected indices for all other dimensions.
    ///
    /// If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
    #[rerun(optional)]
    pub indices: Option<Vec<rerun::components::TensorDimensionIndexSelection>>,

    /// Any dimension listed here will have a slider for the index.
    ///
    /// Edits to the sliders will directly manipulate dimensions on the `indices` list.
    /// If any of the here listed dimensions is equal to `width` or `height`, it will be ignored.
    /// If not specified, adds slides for any dimension in `indices`.
    #[rerun(optional)]
    pub slider: Option<Vec<rerun::blueprint::components::TensorDimensionIndexSlider>>,
}
