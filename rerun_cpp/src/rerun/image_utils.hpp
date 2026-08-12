#pragma once

#include "encodings/channel_datatype.hpp"
#include "encodings/color_model.hpp"
#include "encodings/pixel_format.hpp"
#include "half.hpp"

#include <cassert>
#include <cstdint>

namespace rerun {
    /// The width and height of an image.
    struct WidthHeight {
        uint32_t width;
        uint32_t height;

        WidthHeight(uint32_t width_, uint32_t height_) : width{width_}, height{height_} {}
    };

    /// Number of bits used by this element type
    inline size_t datatype_bits(encodings::ChannelDatatype value) {
        switch (value) {
            case encodings::ChannelDatatype::U8: {
                return 8;
            }
            case encodings::ChannelDatatype::U16: {
                return 16;
            }
            case encodings::ChannelDatatype::U32: {
                return 32;
            }
            case encodings::ChannelDatatype::U64: {
                return 64;
            }
            case encodings::ChannelDatatype::I8: {
                return 8;
            }
            case encodings::ChannelDatatype::I16: {
                return 16;
            }
            case encodings::ChannelDatatype::I32: {
                return 32;
            }
            case encodings::ChannelDatatype::I64: {
                return 64;
            }
            case encodings::ChannelDatatype::F16: {
                return 16;
            }
            case encodings::ChannelDatatype::F32: {
                return 32;
            }
            case encodings::ChannelDatatype::F64: {
                return 64;
            }
            default:
                assert(false && "unreachable");
        }
        return 0;
    }

    inline size_t num_bytes(WidthHeight resolution, encodings::ChannelDatatype datatype) {
        // Widen first: `uint32_t` overflows here, and the result is a buffer length.
        const auto num_pixels =
            static_cast<size_t>(resolution.width) * static_cast<size_t>(resolution.height);
        // rounding upwards:
        return (num_pixels * datatype_bits(datatype) + 7) / 8;
    }

    template <typename TElement>
    inline encodings::ChannelDatatype get_datatype(const TElement* _unused);

    template <>
    inline encodings::ChannelDatatype get_datatype(const uint8_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return encodings::ChannelDatatype::U8;
    }

    template <>
    inline encodings::ChannelDatatype get_datatype(const uint16_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return encodings::ChannelDatatype::U16;
    }

    template <>
    inline encodings::ChannelDatatype get_datatype(const uint32_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return encodings::ChannelDatatype::U32;
    }

    template <>
    inline encodings::ChannelDatatype get_datatype(const uint64_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return encodings::ChannelDatatype::U64;
    }

    template <>
    inline encodings::ChannelDatatype get_datatype(const int8_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return encodings::ChannelDatatype::I8;
    }

    template <>
    inline encodings::ChannelDatatype get_datatype(const int16_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return encodings::ChannelDatatype::I16;
    }

    template <>
    inline encodings::ChannelDatatype get_datatype(const int32_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return encodings::ChannelDatatype::I32;
    }

    template <>
    inline encodings::ChannelDatatype get_datatype(const int64_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return encodings::ChannelDatatype::I64;
    }

    template <>
    inline encodings::ChannelDatatype get_datatype(const rerun::half* _unused) {
        (void)(_unused); // Suppress unused warning.
        return encodings::ChannelDatatype::F16;
    }

    template <>
    inline encodings::ChannelDatatype get_datatype(const float* _unused) {
        (void)(_unused); // Suppress unused warning.
        return encodings::ChannelDatatype::F32;
    }

    template <>
    inline encodings::ChannelDatatype get_datatype(const double* _unused) {
        (void)(_unused); // Suppress unused warning.
        return encodings::ChannelDatatype::F64;
    }

    /// Returns the number of channels for a given color model.
    ///
    /// This is the number of expected elements per pixel.
    inline size_t color_model_channel_count(encodings::ColorModel color_model) {
        switch (color_model) {
            case encodings::ColorModel::L:
                return 1;
            case encodings::ColorModel::BGR:
            case encodings::ColorModel::RGB:
                return 3;
            case encodings::ColorModel::BGRA:
            case encodings::ColorModel::RGBA:
                return 4;
            default:
                assert(false && "unreachable");
        }
        return 0;
    }

    inline size_t pixel_format_num_bytes(
        WidthHeight resolution, encodings::PixelFormat pixel_format
    ) {
        // Widen before multiplying — see `num_bytes` above.
        const auto num_pixels =
            static_cast<size_t>(resolution.width) * static_cast<size_t>(resolution.height);
        switch (pixel_format) {
            // 444 formats.
            case encodings::PixelFormat::Y_U_V24_FullRange:
            case encodings::PixelFormat::Y_U_V24_LimitedRange:
                return num_pixels * 4;

            // 422 formats.
            case encodings::PixelFormat::Y_U_V16_FullRange:
            case encodings::PixelFormat::Y_U_V16_LimitedRange:
            case encodings::PixelFormat::YUY2:
                return 16 * num_pixels / 8;

            // 420 formats.
            case encodings::PixelFormat::Y_U_V12_FullRange:
            case encodings::PixelFormat::Y_U_V12_LimitedRange:
            case encodings::PixelFormat::NV12:
                return 12 * num_pixels / 8;

            // Monochrome formats.
            case encodings::PixelFormat::Y8_LimitedRange:
            case encodings::PixelFormat::Y8_FullRange:
                return num_pixels;

            default:
                assert(false && "unreachable");
        }
        return 0;
    }
} // namespace rerun
