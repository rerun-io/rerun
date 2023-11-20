// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/rgba32.fbs".

#pragma once

#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    /// \private
    template <typename T>
    class NumericBuilder;

    class DataType;
    class UInt32Type;
    using UInt32Builder = NumericBuilder<UInt32Type>;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: An RGBA color with unmultiplied/separate alpha, in sRGB gamma space with linear alpha.
    ///
    /// The color is stored as a 32-bit integer, where the most significant
    /// byte is `R` and the least significant byte is `A`.
    struct Rgba32 {
        uint32_t rgba;

      public:
        // Extensions to generated type defined in 'rgba32_ext.cpp'

        /// Construct Rgba32 from unmultiplied RGBA values.
        Rgba32(uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255)
            : Rgba32(static_cast<uint32_t>((r << 24) | (g << 16) | (b << 8) | a)) {}

        /// Construct Rgba32 from unmultiplied RGBA values.
        Rgba32(const uint8_t (&_rgba)[4]) : Rgba32(_rgba[0], _rgba[1], _rgba[2], _rgba[3]) {}

        /// Construct Rgba32 from RGB values, setting alpha to 255.
        Rgba32(const uint8_t (&_rgb)[3]) : Rgba32(_rgb[0], _rgb[1], _rgb[2]) {}

        uint8_t r() const {
            return static_cast<uint8_t>((rgba >> 24) & 0xFF);
        }

        uint8_t g() const {
            return static_cast<uint8_t>((rgba >> 16) & 0xFF);
        }

        uint8_t b() const {
            return static_cast<uint8_t>((rgba >> 8) & 0xFF);
        }

        uint8_t a() const {
            return static_cast<uint8_t>(rgba & 0xFF);
        }

      public:
        Rgba32() = default;

        Rgba32(uint32_t rgba_) : rgba(rgba_) {}

        Rgba32& operator=(uint32_t rgba_) {
            rgba = rgba_;
            return *this;
        }

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::UInt32Builder* builder, const Rgba32* elements, size_t num_elements
        );
    };
} // namespace rerun::datatypes
