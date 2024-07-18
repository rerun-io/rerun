#include "../error.hpp"
#include "image.hpp"

#include "../collection_adapter_builtins.hpp"

#include <sstream>

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun::archetypes {

#ifdef EDIT_EXTENSION
    // <CODEGEN_COPY_TO_HEADER>

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

    static Image from_color_model_and_bytes(
        components::Resolution2D resolution, components::ColorModel color_model,
        components::ChannelDataType data_type, Collection<uint8_t> bytes
    ) {
        Image img;
        img.data = bytes;
        img.resolution = resolution;
        img.color_model = color_model;
        img.data_type = data_type;
        return img;
    }

    template <typename T>
    static Image from_elements(
        components::Resolution2D resolution, components::ColorModel color_model,
        Collection<T> elements
    ) {
        const auto data_type = get_data_type(elements.data());
        const auto bytes = elements.to_uint8();
        return from_color_model_and_bytes(resolution, color_model, data_type, bytes);
    }

    template <typename T>
    static Image from_elements(
        components::Resolution2D resolution, components::ColorModel color_model,
        std::vector<T> elements
    ) {
        const auto data_type = get_data_type(elements.data());
        const auto bytes = Collection<T>::take_ownership(std::move(elements)).to_uint8();
        return from_color_model_and_bytes(resolution, color_model, data_type, bytes);
    }

    /// Assumes RGB, 8-bit per channel, packed as `RGBRGBRGB…`.
    static Image from_rgb24(components::Resolution2D resolution, Collection<uint8_t> bytes) {
        return Image::from_color_model_and_bytes(
            resolution,
            components::ColorModel::RGB,
            components::ChannelDataType::U8,
            bytes
        );
    }

    /// Assumes RGBA, 8-bit per channel, with separate alpha.
    static Image from_rgba32(components::Resolution2D resolution, Collection<uint8_t> bytes) {
        return Image::from_color_model_and_bytes(
            resolution,
            components::ColorModel::RGBA,
            components::ChannelDataType::U8,
            bytes
        );
    }

    // </CODEGEN_COPY_TO_HEADER>
#endif

} // namespace rerun::archetypes
