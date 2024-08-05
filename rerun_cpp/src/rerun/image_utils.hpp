#pragma once

#include "components/channel_datatype.hpp"
#include "half.hpp"

#include <cstdint>

namespace rerun {
    /// Number of bits used by this element type
    inline size_t datatype_bits(components::ChannelDatatype value) {
        switch (value) {
            case components::ChannelDatatype::U8: {
                return 8;
            }
            case components::ChannelDatatype::U16: {
                return 16;
            }
            case components::ChannelDatatype::U32: {
                return 32;
            }
            case components::ChannelDatatype::U64: {
                return 64;
            }
            case components::ChannelDatatype::I8: {
                return 8;
            }
            case components::ChannelDatatype::I16: {
                return 16;
            }
            case components::ChannelDatatype::I32: {
                return 32;
            }
            case components::ChannelDatatype::I64: {
                return 64;
            }
            case components::ChannelDatatype::F16: {
                return 16;
            }
            case components::ChannelDatatype::F32: {
                return 32;
            }
            case components::ChannelDatatype::F64: {
                return 64;
            }
        }
        return 0;
    }

    inline size_t num_bytes(
        components::Resolution2D resolution, components::ChannelDatatype datatype
    ) {
        const size_t width = static_cast<size_t>(resolution.width());
        const size_t height = static_cast<size_t>(resolution.height());
        return (width * height * datatype_bits(datatype) + 7) / 8; // rounding upwards
    }

    template <typename TElement>
    inline components::ChannelDatatype get_datatype(const TElement* _unused);

    template <>
    inline components::ChannelDatatype get_datatype(const uint8_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDatatype::U8;
    }

    template <>
    inline components::ChannelDatatype get_datatype(const uint16_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDatatype::U16;
    }

    template <>
    inline components::ChannelDatatype get_datatype(const uint32_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDatatype::U32;
    }

    template <>
    inline components::ChannelDatatype get_datatype(const uint64_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDatatype::U64;
    }

    template <>
    inline components::ChannelDatatype get_datatype(const int8_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDatatype::I8;
    }

    template <>
    inline components::ChannelDatatype get_datatype(const int16_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDatatype::I16;
    }

    template <>
    inline components::ChannelDatatype get_datatype(const int32_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDatatype::I32;
    }

    template <>
    inline components::ChannelDatatype get_datatype(const int64_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDatatype::I64;
    }

    template <>
    inline components::ChannelDatatype get_datatype(const rerun::half* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDatatype::F16;
    }

    template <>
    inline components::ChannelDatatype get_datatype(const float* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDatatype::F32;
    }

    template <>
    inline components::ChannelDatatype get_datatype(const double* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDatatype::F64;
    }
} // namespace rerun
