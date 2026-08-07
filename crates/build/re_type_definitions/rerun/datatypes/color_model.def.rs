// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// Specified what color components are present in an [archetypes.Image].
///
/// This combined with [datatypes.ChannelDatatype] determines the pixel format of an image.
#[rerun::rerun_type]
#[repr(u8)]
#[rerun(state = "stable")]
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
