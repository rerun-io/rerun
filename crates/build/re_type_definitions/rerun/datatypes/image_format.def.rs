// This is a Rerun type definition for the SDK, not executable code.
// It is parsed by `re_types_builder` to generate the Rust, Python and C++ bindings.

/// The metadata describing the contents of a [components.ImageBuffer].
#[rerun::rerun_type]
#[rust(derive(Default, Copy, PartialEq, Eq, Hash))]
#[rerun(state = "stable")]
pub struct ImageFormat {
    /// The width of the image in pixels.
    pub width: u32,

    /// The height of the image in pixels.
    pub height: u32,

    /// Used mainly for chroma downsampled formats and differing number of bits per channel.
    ///
    /// If specified, this takes precedence over both [datatypes.ColorModel] and [datatypes.ChannelDatatype] (which are ignored).
    pub pixel_format: Option<rerun::datatypes::PixelFormat>,

    /// L, RGB, RGBA, …
    ///
    /// Also requires a [datatypes.ChannelDatatype] to fully specify the pixel format.
    pub color_model: Option<rerun::datatypes::ColorModel>,

    /// The data type of each channel (e.g. the red channel) of the image data (U8, F16, …).
    ///
    /// Also requires a [datatypes.ColorModel] to fully specify the pixel format.
    pub channel_datatype: Option<rerun::datatypes::ChannelDatatype>,
}
