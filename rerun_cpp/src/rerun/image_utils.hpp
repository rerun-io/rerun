#pragma once

#include "components/element_type.hpp"
#include "half.hpp"

namespace rerun {
    /// Number of bits used by this element type
    inline size_t element_type_bits(components::ElementType value) {
        switch (value) {
            case components::ElementType::U8: {
                return 8;
            }
            case components::ElementType::U16: {
                return 16;
            }
            case components::ElementType::U32: {
                return 32;
            }
            case components::ElementType::U64: {
                return 64;
            }
            case components::ElementType::I8: {
                return 8;
            }
            case components::ElementType::I16: {
                return 16;
            }
            case components::ElementType::I32: {
                return 32;
            }
            case components::ElementType::I64: {
                return 64;
            }
            case components::ElementType::F16: {
                return 16;
            }
            case components::ElementType::F32: {
                return 32;
            }
            case components::ElementType::F64: {
                return 64;
            }
        }
    }

    inline size_t num_bytes(
        components::Resolution2D resolution, components::ElementType element_type
    ) {
        const size_t width = static_cast<size_t>(resolution.width());
        const size_t height = static_cast<size_t>(resolution.height());
        return width * height * element_type_bits(element_type) / 8;
    }

    template <typename TElement>
    inline components::ElementType get_element_type(const TElement* _unused);

    template <>
    inline components::ElementType get_element_type(const uint8_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ElementType::U8;
    }

    template <>
    inline components::ElementType get_element_type(const uint16_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ElementType::U16;
    }

    template <>
    inline components::ElementType get_element_type(const uint32_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ElementType::U32;
    }

    template <>
    inline components::ElementType get_element_type(const uint64_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ElementType::U64;
    }

    template <>
    inline components::ElementType get_element_type(const int8_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ElementType::I8;
    }

    template <>
    inline components::ElementType get_element_type(const int16_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ElementType::I16;
    }

    template <>
    inline components::ElementType get_element_type(const int32_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ElementType::I32;
    }

    template <>
    inline components::ElementType get_element_type(const int64_t* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ElementType::I64;
    }

    template <>
    inline components::ElementType get_element_type(const rerun::half* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ElementType::F16;
    }

    template <>
    inline components::ElementType get_element_type(const float* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ElementType::F32;
    }

    template <>
    inline components::ElementType get_element_type(const double* _unused) {
        (void)(_unused); // Suppress unused warning.
        return components::ElementType::F64;
    }
} // namespace rerun
