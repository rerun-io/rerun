// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Draw order of 2D elements. Higher values are drawn on top of lower values.
///
/// An entity can have only a single draw order component.
/// Within an entity draw order is governed by the order of the components.
///
/// Draw order for entities with the same draw order is generally undefined.
///
/// If no draw order is set, the viewer uses these defaults:
/// * -20: [`rerun::archetypes::DepthImage`], [`rerun::archetypes::EncodedDepthImage`]
/// * -15: [`rerun::archetypes::VideoFrameReference`], [`rerun::archetypes::VideoStream`]
/// * -10: [`rerun::archetypes::Image`], [`rerun::archetypes::EncodedImage`], [`rerun::archetypes::GridMap`]
/// * 0: [`rerun::archetypes::SegmentationImage`]
/// * 10: [`rerun::archetypes::Boxes2D`], [`rerun::archetypes::Ellipses2D`]
/// * 20: [`rerun::archetypes::Arrows2D`], [`rerun::archetypes::LineStrips2D`]
/// * 30: [`rerun::archetypes::Points2D`]
///
/// For example, to draw an image on top of a segmentation image, give the image a draw order above 0.
#[rerun::rerun_type]
#[python(aliases = "float")]
#[python(array_aliases = "float | npt.NDArray[np.float32]")]
#[rust(derive(Copy))]
#[rust(repr = "transparent")]
#[rerun(state = "stable")]
pub struct DrawOrder {
    pub value: rerun::encodings::Float32,
}
