#pragma once

#include "components/channel_data_type.hpp"
#include "half.hpp"

#include <cstdint>

namespace rerun {
    /// Number of bits used by this element type
    inline size_t data_type_bits(components::ChannelDataType value) {
        switch (value) {
            case components::ChannelDataType::U8: {
                return 8;
            }
            case components::ChannelDataType::U16: {
                return 16;
            }
            case components::ChannelDataType::U32: {
                return 32;
            }
            case components::ChannelDataType::U64: {
                return 64;
            }
            case components::ChannelDataType::I8: {
                return 8;
            }
            case components::ChannelDataType::I16: {
                return 16;
            }
            case components::ChannelDataType::I32: {
                return 32;
            }
            case components::ChannelDataType::I64: {
                return 64;
            }
            case components::ChannelDataType::F16: {
                return 16;
            }
            case components::ChannelDataType::F32: {
                return 32;
            }
            case components::ChannelDataType::F64: {
                return 64;
            }
        }
        return 0;
    }

    inline size_t num_bytes(
        components::Resolution2D resolution, components::ChannelDataType data_type
    ) {
        const size_t width = static_cast<size_t>(resolution.width());
        const size_t height = static_cast<size_t>(resolution.height());
        return (width * height * data_type_bits(data_type) + 7) / 8; // rounding upwards
    }

    template <typename TElement>
    inline components::ChannelDataType get_data_type(const TElement* _unused);

    template <>
    inline components::ChannelDataType get_data_type(const uint8_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDataType::U8;
    }

    template <>
    inline components::ChannelDataType get_data_type(const uint16_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDataType::U16;
    }

    template <>
    inline components::ChannelDataType get_data_type(const uint32_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDataType::U32;
    }

    template <>
    inline components::ChannelDataType get_data_type(const uint64_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDataType::U64;
    }

    template <>
    inline components::ChannelDataType get_data_type(const int8_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDataType::I8;
    }

    template <>
    inline components::ChannelDataType get_data_type(const int16_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDataType::I16;
    }

    template <>
    inline components::ChannelDataType get_data_type(const int32_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDataType::I32;
    }

    template <>
    inline components::ChannelDataType get_data_type(const int64_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDataType::I64;
    }

    template <>
    inline components::ChannelDataType get_data_type(const rerun::half* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDataType::F16;
    }

    template <>
    inline components::ChannelDataType get_data_type(const float* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDataType::F32;
    }

    template <>
    inline components::ChannelDataType get_data_type(const double* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ChannelDataType::F64;
    }
} // namespace rerun
