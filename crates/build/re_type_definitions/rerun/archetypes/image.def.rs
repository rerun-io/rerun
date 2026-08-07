// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// A monochrome or color image.
///
/// See also [archetypes.DepthImage] and [archetypes.SegmentationImage].
///
/// Rerun also supports compressed images (JPEG, PNG, …), using [archetypes.EncodedImage].
/// For images that refer to video frames see [archetypes.VideoFrameReference].
/// Compressing images or using video data instead can save a lot of bandwidth and memory.
///
/// The raw image data is stored as a single buffer of bytes in a [components.Blob].
/// The meaning of these bytes is determined by the [components.ImageFormat] which specifies the resolution
/// and the pixel format (e.g. RGB, RGBA, …).
///
/// The order of dimensions in the underlying [components.Blob] follows the typical
/// row-major, interleaved-pixel image format.
///
/// \cpp Since the underlying [rerun::components::Blob] uses `rerun::Collection` internally,
/// \cpp data can be passed in without a copy from raw pointers or by reference from `std::vector`/`std::array`/c-arrays.
/// \cpp If needed, this "borrow-behavior" can be extended by defining your own `rerun::CollectionAdapter`.
///
/// \example archetypes/image_simple image="https://static.rerun.io/image_simple/06ba7f8582acc1ffb42a7fd0006fad7816f3e4e4/1200w.png"
/// \example archetypes/image_formats title="Logging images with various formats" image="https://static.rerun.io/image_formats/182a233fb4d0680eb31912a82f328ddaaa66324e/1200w.png"
/// \example archetypes/image_advanced !api title="Image from file, PIL & OpenCV" image="https://static.rerun.io/image_advanced/7ea3e3876858879bf16d6efe6de313f7b2403881/1200w.png"
/// \example archetypes/image_row_updates !api title="Update an image over time" image="https://static.rerun.io/image_column_updates/8edcdc512f7b97402f03c24d7dcbe01b3651f86d/1200w.png"
/// \example archetypes/image_column_updates !api title="Update an image over time, in a single operation" image="https://static.rerun.io/image_column_updates/8edcdc512f7b97402f03c24d7dcbe01b3651f86d/1200w.png"
#[rerun::rerun_type]
#[cpp(no_field_ctors)]
#[docs(category = "Image & tensor")]
#[docs(view_types = "Spatial2DView, Spatial3DView: if logged under a projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "Image")]
#[rust(derive(PartialEq))]
pub struct Image {
    /// The raw image data.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub buffer: rerun::components::ImageBuffer,

    /// The format of the image.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub format: rerun::components::ImageFormat,

    /// Opacity of the image, useful for layering several media.
    ///
    /// Defaults to 1.0 (fully opaque).
    #[rerun(optional)]
    pub opacity: Option<rerun::components::Opacity>,

    /// An optional floating point value that specifies the 2D drawing order.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    /// Defaults to `-10.0`.
    #[rerun(optional)]
    pub draw_order: Option<rerun::components::DrawOrder>,

    /// Optional filter used when a texel is magnified (displayed larger than a screen pixel).
    #[rerun(optional)]
    pub magnification_filter: Option<rerun::components::MagnificationFilter>,
}
