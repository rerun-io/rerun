// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// An image encoded as e.g. a JPEG or PNG.
///
/// Rerun also supports uncompressed images with the [archetypes.Image].
/// For images that refer to video frames see [archetypes.VideoFrameReference].
///
/// \py To compress an image, use [`rerun.Image.compress`][].
///
/// \example archetypes/encoded_image title="Encoded image" image="https://static.rerun.io/encoded_image/6e92868b6533be5fb2dfd9e26938eb7a256bfb01/1200w.png"
#[rerun::rerun_type]
#[cpp(no_field_ctors)]
#[docs(category = "Image & tensor")]
#[docs(view_types = "Spatial2DView, Spatial3DView: if logged under a projection")]
#[rerun(state = "stable")]
#[rerun(visualizer = "EncodedImage")]
#[rust(derive(PartialEq))]
pub struct EncodedImage {
    /// The encoded content of some image file, e.g. a PNG or JPEG.
    #[rerun(no_ui_edit)]
    #[rerun(required)]
    pub blob: rerun::components::Blob,

    /// The Media Type of the asset.
    ///
    /// Supported values:
    /// * `image/jpeg`
    /// * `image/png`
    ///
    /// If omitted, the viewer will try to guess from the data blob.
    /// If it cannot guess, it won't be able to render the asset.
    #[rerun(recommended)]
    pub media_type: Option<rerun::components::MediaType>,

    /// Opacity of the image, useful for layering several media.
    ///
    /// Defaults to 1.0 (fully opaque).
    #[rerun(optional)]
    pub opacity: Option<rerun::components::Opacity>,

    /// An optional floating point value that specifies the 2D drawing order.
    ///
    /// Objects with higher values are drawn on top of those with lower values.
    #[rerun(optional)]
    pub draw_order: Option<rerun::components::DrawOrder>,

    /// Optional filter used when a texel is magnified (displayed larger than a screen pixel).
    #[rerun(optional)]
    pub magnification_filter: Option<rerun::components::MagnificationFilter>,
}
