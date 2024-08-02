#include "../error.hpp"
#include "image.hpp"

#include "../collection_adapter_builtins.hpp"

#include <sstream>

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun::archetypes {

#ifdef EDIT_EXTENSION
    // <CODEGEN_COPY_TO_HEADER>

    /// Construct an image from resolution, pixel format and bytes.
    ///
    /// @param bytes The raw image data.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * pixel_format.bytes_per_pixel`.
    static Image from_pixel_format(
        components::Resolution2D resolution, components::PixelFormat pixel_format,
        Collection<uint8_t> bytes
    ) {
        Image img;
        img.data = bytes;
        img.resolution = resolution;
        img.pixel_format = pixel_format;
        return img;
    }

    /// Construct an image from resolution, color model, channel datatype and bytes.
    ///
    /// @param bytes The raw image data.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * datatype.bytes * color_model.num_channels`.
    static Image from_color_model_and_bytes(
        components::Resolution2D resolution, components::ColorModel color_model,
        components::ChannelDatatype datatype, Collection<uint8_t> bytes
    ) {
        Image img;
        img.data = bytes;
        img.resolution = resolution;
        img.color_model = color_model;
        img.datatype = datatype;
        return img;
    }

    /// Construct an image from resolution, color model and elements,
    /// inferring the channel datatype from the element type.
    ///
    /// @param elements Pixel data as a `rerun::Collection`.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * color_model.num_channels`.
    template <typename T>
    static Image from_elements(
        components::Resolution2D resolution, components::ColorModel color_model,
        Collection<T> elements
    ) {
        const auto datatype = get_datatype(elements.data());
        const auto bytes = elements.to_uint8();
        return from_color_model_and_bytes(resolution, color_model, datatype, bytes);
    }

    /// Assumes single channel greyscale/luminance with 8-bit per value.
    ///
    /// @param bytes Pixel data as a `rerun::Collection`.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H`.
    static Image from_greyscale8(components::Resolution2D resolution, Collection<uint8_t> bytes) {
        return Image::from_color_model_and_bytes(
            resolution,
            components::ColorModel::L,
            components::ChannelDatatype::U8,
            bytes
        );
    }

    /// Assumes RGB, 8-bit per channel, packed as `RGBRGBRGB…`.
    ///
    /// @param bytes Pixel data as a `rerun::Collection`.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * 3`.
    static Image from_rgb24(components::Resolution2D resolution, Collection<uint8_t> bytes) {
        return Image::from_color_model_and_bytes(
            resolution,
            components::ColorModel::RGB,
            components::ChannelDatatype::U8,
            bytes
        );
    }

    /// Assumes RGBA, 8-bit per channel, with separate alpha.
    ///
    /// @param bytes Pixel data as a `rerun::Collection`.
    /// If the data does not outlive the image, use `std::move` or create the `rerun::Collection`
    /// explicitly ahead of time with `rerun::Collection::take_ownership`.
    /// The length of the data should be `W * H * 4`.
    static Image from_rgba32(components::Resolution2D resolution, Collection<uint8_t> bytes) {
        return Image::from_color_model_and_bytes(
            resolution,
            components::ColorModel::RGBA,
            components::ChannelDatatype::U8,
            bytes
        );
    }

    // </CODEGEN_COPY_TO_HEADER>
#endif

} // namespace rerun::archetypes
