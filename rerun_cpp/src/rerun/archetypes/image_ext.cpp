#if 0

#include "image.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../image_utils.hpp"

// </CODEGEN_COPY_TO_HEADER>

namespace rerun::archetypes {

    // <CODEGEN_COPY_TO_HEADER>

    /// Construct an image from resolution, pixel format and bytes.
    ///
    /// @param bytes The raw image data as bytes.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * pixel_format.bytes_per_pixel`.
    /// @param resolution_ The resolution of the image.
    /// @param pixel_format_ How the data should be interpreted.
    Image(
        Collection<uint8_t> bytes, components::Resolution2D resolution_,
        components::PixelFormat pixel_format_
    )
        : data(std::move(bytes)), resolution(resolution_), pixel_format(pixel_format_) {}

    /// Construct an image from resolution, color model, channel datatype and bytes.
    ///
    /// @param bytes The raw image data.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * datatype.bytes * color_model.num_channels`.
    /// @param resolution_ The resolution of the image.
    /// @param color_model_ The color model of the pixel data.
    /// @param datatype_ Datatype of the individual channels of the color model.
    Image(
        Collection<uint8_t> bytes, components::Resolution2D resolution_,
        components::ColorModel color_model_, components::ChannelDatatype datatype_
    )
        : data(std::move(bytes)),
          resolution(resolution_),
          color_model(color_model_),
          datatype(datatype_) {}

    /// Construct an image from resolution, color model and elements,
    /// inferring the channel datatype from the element type.
    ///
    /// @param elements Pixel data as a `rerun::Collection`.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * color_model.num_channels`.
    /// @param resolution_ The resolution of the image.
    /// @param color_model_ The color model of the pixel data.
    /// Each element in elements is interpreted as a single channel of the color model.
    template <typename T>
    Image(
        Collection<T> elements, components::Resolution2D resolution_,
        components::ColorModel color_model_
    )
        : Image(elements.to_uint8(), resolution_, color_model_, get_datatype(elements.data())) {}

    /// Construct an image from resolution, color model and element pointer,
    /// inferring the channel datatype from the element type.
    ///
    /// @param elements The raw image data.
    /// ⚠️ Does not take ownership of the data, the caller must ensure the data outlives the image.
    /// The number of elements is assumed to be `W * H * color_model.num_channels`.
    /// @param resolution_ The resolution of the image.
    /// @param color_model_ The color model of the pixel data.
    /// Each element in elements is interpreted as a single channel of the color model.
    template <typename T>
    Image(
        const T* elements, components::Resolution2D resolution_, components::ColorModel color_model_
    )
        : Image(
              rerun::Collection<uint8_t>::borrow(
                  reinterpret_cast<const uint8_t*>(elements),
                  resolution_.width() * resolution_.height() * color_model_channel_count(color_model_)
              ),
              resolution_, color_model_, get_datatype(elements)
          ) {}

    /// Assumes single channel greyscale/luminance with 8-bit per value.
    ///
    /// @param bytes Pixel data as a `rerun::Collection`.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H`.
    /// @param resolution The resolution of the image.
    static Image from_greyscale8(Collection<uint8_t> bytes, components::Resolution2D resolution) {
        return Image(bytes, resolution, components::ColorModel::L, components::ChannelDatatype::U8);
    }

    /// Assumes RGB, 8-bit per channel, packed as `RGBRGBRGB…`.
    ///
    /// @param bytes Pixel data as a `rerun::Collection`.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * 3`.
    /// @param resolution The resolution of the image.
    static Image from_rgb24(Collection<uint8_t> bytes, components::Resolution2D resolution) {
        return Image(
            bytes,
            resolution,
            components::ColorModel::RGB,
            components::ChannelDatatype::U8
        );
    }

    /// Assumes RGBA, 8-bit per channel, with separate alpha.
    ///
    /// @param bytes Pixel data as a `rerun::Collection`.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * 4`.
    /// @param resolution The resolution of the image.
    static Image from_rgba32(Collection<uint8_t> bytes, components::Resolution2D resolution) {
        return Image(
            bytes,
            resolution,
            components::ColorModel::RGBA,
            components::ChannelDatatype::U8
        );
    }

    // </CODEGEN_COPY_TO_HEADER>

} // namespace rerun::archetypes

#endif
