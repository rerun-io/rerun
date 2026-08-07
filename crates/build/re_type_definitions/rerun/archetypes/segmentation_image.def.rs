// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// An image made up of integer [components.ClassId]s.
///
/// Each pixel corresponds to a [components.ClassId] that will be mapped to a color based on [archetypes.AnnotationContext].
///
/// In the case of floating point images, the label will be looked up based on rounding to the nearest
/// integer value.
///
/// Use [archetypes.AnnotationContext] to associate each class with a color and a label.
///
/// \cpp Since the underlying `rerun::datatypes::TensorData` uses `rerun::Collection` internally,
/// \cpp data can be passed in without a copy from raw pointers or by reference from `std::vector`/`std::array`/c-arrays.
/// \cpp If needed, this "borrow-behavior" can be extended by defining your own `rerun::CollectionAdapter`.
///
/// \example archetypes/segmentation_image_simple title="Simple segmentation image" image="https://static.rerun.io/segmentation_image_simple/f8aac62abcf4c59c5d62f9ebc2d86fd0285c1736/1200w.png"
#[rerun::rerun_type]
#[cpp(no_field_ctors)]
#[docs(category = "Image & tensor")]
#[docs(view_types = "Spatial2DView, Spatial3DView: if logged under a projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "SegmentationImage")]
#[rust(derive(PartialEq))]
pub struct SegmentationImage {
    /// The raw image data.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub buffer: rerun::components::ImageBuffer,

    /// The format of the image.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub format: rerun::components::ImageFormat,

    /// Opacity of the image, useful for layering the segmentation image on top of another image.
    ///
    /// Defaults to 0.5 if there's any other images in the scene, otherwise 1.0.
    #[rerun(optional)]
    pub opacity: Option<rerun::components::Opacity>,

    /// An optional floating point value that specifies the 2D drawing order.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    /// Defaults to `0.0`.
    #[rerun(optional)]
    pub draw_order: Option<rerun::components::DrawOrder>,
}
