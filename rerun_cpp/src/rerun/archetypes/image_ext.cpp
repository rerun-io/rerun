#if 0

#include "image.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../image_utils.hpp"

// </CODEGEN_COPY_TO_HEADER>

namespace rerun::archetypes {

    // <CODEGEN_COPY_TO_HEADER>

    /// Construct an image from bytes and image format.
    ///
    /// @param bytes The raw image data as bytes.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * image_format.bytes_per_pixel`.
    /// @param format_ How the data should be interpreted.
    Image(
        Collection<uint8_t> bytes, components::ImageFormat format_
    ) {
        if (bytes.size() != format_.image_format.num_bytes()) {
            Error(
                ErrorCode::InvalidTensorDimension,
                "Image buffer has the wrong size. Got " + std::to_string(bytes.size()) +
                    " bytes, expected " + std::to_string(format_.image_format.num_bytes())
            )
                .handle();
        }
        *this = std::move(*this).with_buffer(bytes).with_format(format_);
    }
    /// Construct an image from resolution, pixel format and bytes.
    ///
    /// @param bytes The raw image data as bytes.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * pixel_format.bytes_per_pixel`.
    /// @param resolution The resolution of the image as {width, height}.
    /// @param pixel_format How the data should be interpreted.
    Image(
        Collection<uint8_t> bytes, WidthHeight resolution,
        datatypes::PixelFormat pixel_format
    )
        : Image{std::move(bytes), datatypes::ImageFormat{resolution, pixel_format}} {}

    /// Construct an image from resolution, color model, channel datatype and bytes.
    ///
    /// @param bytes The raw image data.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * datatype.bytes * color_model.num_channels`.
    /// @param resolution The resolution of the image as {width, height}.
    /// @param color_model The color model of the pixel data.
    /// @param datatype Datatype of the individual channels of the color model.
    Image(
        Collection<uint8_t> bytes, WidthHeight resolution,
        datatypes::ColorModel color_model, datatypes::ChannelDatatype datatype
    )
        : Image(std::move(bytes), datatypes::ImageFormat(resolution, color_model, datatype)) {}

    /// Construct an image from resolution, color model and elements,
    /// inferring the channel datatype from the element type.
    ///
    /// @param elements Pixel data as a `rerun::Collection`.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * color_model.num_channels`.
    /// @param resolution The resolution of the image as {width, height}.
    /// @param color_model The color model of the pixel data.
    /// Each element in elements is interpreted as a single channel of the color model.
    template <typename T>
    Image(
        Collection<T> elements, WidthHeight resolution,
        datatypes::ColorModel color_model
    )
        : Image(elements.to_uint8(), resolution, color_model, get_datatype(elements.data())) {}

    /// Construct an image from resolution, color model and element pointer,
    /// inferring the channel datatype from the element type.
    ///
    /// @param elements The raw image data.
    /// ⚠️ Does not take ownership of the data, the caller must ensure the data outlives the image.
    /// The number of elements is assumed to be `W * H * color_model.num_channels`.
    /// @param resolution The resolution of the image as {width, height}.
    /// @param color_model The color model of the pixel data.
    /// Each element in elements is interpreted as a single channel of the color model.
    template <typename T>
    Image(
        const T* elements, WidthHeight resolution, datatypes::ColorModel color_model
    )
        : Image(
              rerun::Collection<uint8_t>::borrow(
                  reinterpret_cast<const uint8_t*>(elements),
                  resolution.width * resolution.height * color_model_channel_count(color_model)
              ),
              resolution, color_model, get_datatype(elements)
          ) {}

    /// Assumes single channel grayscale/luminance with 8-bit per value.
    ///
    /// @param bytes Pixel data as a `rerun::Collection`.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H`.
    /// @param resolution The resolution of the image as {width, height}.
    static Image from_grayscale8(Collection<uint8_t> bytes, WidthHeight resolution) {
        return Image(bytes, resolution, datatypes::ColorModel::L, datatypes::ChannelDatatype::U8);
    }

    /// Assumes single channel grayscale/luminance with 8-bit per value.
    ///
    /// @param bytes Pixel data as a `rerun::Collection`.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H`.
    /// @param resolution The resolution of the image as {width, height}.
    [[deprecated("Renamed `from_grayscale8`")]]
    static Image from_greyscale8(Collection<uint8_t> bytes, WidthHeight resolution) {
        return Image(
            bytes,
            resolution,
            datatypes::ColorModel::L,
            datatypes::ChannelDatatype::U8
        );
    }

    /// Assumes RGB, 8-bit per channel, packed as `RGBRGBRGB…`.
    ///
    /// @param bytes Pixel data as a `rerun::Collection`.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * 3`.
    /// @param resolution The resolution of the image as {width, height}.
    static Image from_rgb24(Collection<uint8_t> bytes, WidthHeight resolution) {
        return Image(
            bytes,
            resolution,
            datatypes::ColorModel::RGB,
            datatypes::ChannelDatatype::U8
        );
    }

    /// Assumes RGBA, 8-bit per channel, with separate alpha.
    ///
    /// @param bytes Pixel data as a `rerun::Collection`.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * 4`.
    /// @param resolution The resolution of the image as {width, height}.
    static Image from_rgba32(Collection<uint8_t> bytes, WidthHeight resolution) {
        return Image(
            bytes,
            resolution,
            datatypes::ColorModel::RGBA,
            datatypes::ChannelDatatype::U8
        );
    }

    // </CODEGEN_COPY_TO_HEADER>

} // namespace rerun::archetypes

#endif
