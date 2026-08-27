// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Specifies what color components are present in an [`rerun::archetypes::Image`].
///
/// This combined with [`rerun::encodings::ChannelDatatype`] determines the pixel format of an image.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
#[rust(arrow_opt)]
pub enum ColorModel {
    /// Grayscale luminance intencity/brightness/value, sometimes called `Y`
    #[default]
    L = 1,

    /// Red, Green, Blue
    RGB = 2,

    /// Red, Green, Blue, Alpha
    RGBA = 3,

    /// Blue, Green, Red
    BGR = 4,

    /// Blue, Green, Red, Alpha
    BGRA = 5,
}
