#pragma once

#include "datatypes/channel_datatype.hpp"
#include "datatypes/color_model.hpp"
#include "datatypes/pixel_format.hpp"
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
    inline size_t datatype_bits(datatypes::ChannelDatatype value) {
        switch (value) {
            case datatypes::ChannelDatatype::U8: {
                return 8;
            }
            case datatypes::ChannelDatatype::U16: {
                return 16;
            }
            case datatypes::ChannelDatatype::U32: {
                return 32;
            }
            case datatypes::ChannelDatatype::U64: {
                return 64;
            }
            case datatypes::ChannelDatatype::I8: {
                return 8;
            }
            case datatypes::ChannelDatatype::I16: {
                return 16;
            }
            case datatypes::ChannelDatatype::I32: {
                return 32;
            }
            case datatypes::ChannelDatatype::I64: {
                return 64;
            }
            case datatypes::ChannelDatatype::F16: {
                return 16;
            }
            case datatypes::ChannelDatatype::F32: {
                return 32;
            }
            case datatypes::ChannelDatatype::F64: {
                return 64;
            }
            default:
                assert(false && "unreachable");
        }
        return 0;
    }

    inline size_t num_bytes(WidthHeight resolution, datatypes::ChannelDatatype datatype) {
        // rounding upwards:
        return (resolution.width * resolution.height * datatype_bits(datatype) + 7) / 8;
    }

    template <typename TElement>
    inline datatypes::ChannelDatatype get_datatype(const TElement* _unused);

    template <>
    inline datatypes::ChannelDatatype get_datatype(const uint8_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return datatypes::ChannelDatatype::U8;
    }

    template <>
    inline datatypes::ChannelDatatype get_datatype(const uint16_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return datatypes::ChannelDatatype::U16;
    }

    template <>
    inline datatypes::ChannelDatatype get_datatype(const uint32_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return datatypes::ChannelDatatype::U32;
    }

    template <>
    inline datatypes::ChannelDatatype get_datatype(const uint64_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return datatypes::ChannelDatatype::U64;
    }

    template <>
    inline datatypes::ChannelDatatype get_datatype(const int8_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return datatypes::ChannelDatatype::I8;
    }

    template <>
    inline datatypes::ChannelDatatype get_datatype(const int16_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return datatypes::ChannelDatatype::I16;
    }

    template <>
    inline datatypes::ChannelDatatype get_datatype(const int32_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return datatypes::ChannelDatatype::I32;
    }

    template <>
    inline datatypes::ChannelDatatype get_datatype(const int64_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return datatypes::ChannelDatatype::I64;
    }

    template <>
    inline datatypes::ChannelDatatype get_datatype(const rerun::half* _unused) {
        (void)(_unused); // Suppress unused warning.
        return datatypes::ChannelDatatype::F16;
    }

    template <>
    inline datatypes::ChannelDatatype get_datatype(const float* _unused) {
        (void)(_unused); // Suppress unused warning.
        return datatypes::ChannelDatatype::F32;
    }

    template <>
    inline datatypes::ChannelDatatype get_datatype(const double* _unused) {
        (void)(_unused); // Suppress unused warning.
        return datatypes::ChannelDatatype::F64;
    }

    /// Returns the number of channels for a given color model.
    ///
    /// This is the number of expected elements per pixel.
    inline size_t color_model_channel_count(datatypes::ColorModel color_model) {
        switch (color_model) {
            case datatypes::ColorModel::L:
                return 1;
            case datatypes::ColorModel::BGR:
            case datatypes::ColorModel::RGB:
                return 3;
            case datatypes::ColorModel::BGRA:
            case datatypes::ColorModel::RGBA:
                return 4;
            default:
                assert(false && "unreachable");
        }
        return 0;
    }

    inline size_t pixel_format_num_bytes(
        WidthHeight resolution, datatypes::PixelFormat pixel_format
    ) {
        auto num_pixels = resolution.width * resolution.height;
        switch (pixel_format) {
            // 444 formats.
            case datatypes::PixelFormat::Y_U_V24_FullRange:
            case datatypes::PixelFormat::Y_U_V24_LimitedRange:
                return num_pixels * 4;

            // 422 formats.
            case datatypes::PixelFormat::Y_U_V16_FullRange:
            case datatypes::PixelFormat::Y_U_V16_LimitedRange:
            case datatypes::PixelFormat::YUY2:
                return 16 * num_pixels / 8;

            // 420 formats.
            case datatypes::PixelFormat::Y_U_V12_FullRange:
            case datatypes::PixelFormat::Y_U_V12_LimitedRange:
            case datatypes::PixelFormat::NV12:
                return 12 * num_pixels / 8;

            // Monochrome formats.
            case datatypes::PixelFormat::Y8_LimitedRange:
            case datatypes::PixelFormat::Y8_FullRange:
                return num_pixels;

            default:
                assert(false && "unreachable");
        }
        return 0;
    }
} // namespace rerun
