#if 0

namespace rerun::components {
    // <CODEGEN_COPY_TO_HEADER>

    /// From a specific pixel format.
    ImageFormat(rerun::WidthHeight resolution, encodings::PixelFormat pixel_format)
        : image_format(resolution, pixel_format) {}

    /// Create a new image format for depth or segmentation images with the given resolution and datatype.
    ImageFormat(rerun::WidthHeight resolution, encodings::ChannelDatatype datatype)
        : image_format(resolution, datatype) {}

    ImageFormat(
        rerun::WidthHeight resolution, encodings::ColorModel color_model,
        encodings::ChannelDatatype datatype
    )
        : image_format(resolution, color_model, datatype) {}

    // </CODEGEN_COPY_TO_HEADER>
} // namespace rerun::components

#endif
