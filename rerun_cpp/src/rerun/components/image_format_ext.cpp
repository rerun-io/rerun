#if 0

namespace rerun::components {
    // <CODEGEN_COPY_TO_HEADER>

    /// From a specific pixel format.
    ImageFormat(rerun::WidthHeight resolution, datatypes::PixelFormat pixel_format)
        : image_format(resolution, pixel_format) {}

    /// Create a new image format for depth or segmentation images with the given resolution and datatype.
    ImageFormat(rerun::WidthHeight resolution, datatypes::ChannelDatatype datatype)
        : image_format(resolution, datatype) {}

    ImageFormat(
        rerun::WidthHeight resolution, datatypes::ColorModel color_model,
        datatypes::ChannelDatatype datatype
    )
        : image_format(resolution, color_model, datatype) {}

    // </CODEGEN_COPY_TO_HEADER>
} // namespace rerun::components

#endif
