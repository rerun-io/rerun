// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/components/color.fbs".

#pragma once

#include "../datatypes/rgba32.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace rerun::components {
    /// **Component**: An RGBA color with unmultiplied/separate alpha, in sRGB gamma space with linear alpha.
    ///
    /// The color is stored as a 32-bit integer, where the most significant
    /// byte is `R` and the least significant byte is `A`.
    struct Color {
        rerun::datatypes::Rgba32 rgba;

      public:
        // Extensions to generated type defined in 'color_ext.cpp'

        /// Construct Color from unmultiplied RGBA values.
        Color(uint8_t r, uint8_t g, uint8_t b, uint8_t a = 255) : rgba(r, g, b, a) {}

        uint8_t r() const {
            return rgba.r();
        }

        uint8_t g() const {
            return rgba.g();
        }

        uint8_t b() const {
            return rgba.b();
        }

        uint8_t a() const {
            return rgba.a();
        }

      public:
        Color() = default;

        Color(rerun::datatypes::Rgba32 rgba_) : rgba(rgba_) {}

        Color& operator=(rerun::datatypes::Rgba32 rgba_) {
            rgba = rgba_;
            return *this;
        }

        Color(uint32_t rgba_) : rgba(rgba_) {}

        Color& operator=(uint32_t rgba_) {
            rgba = rgba_;
            return *this;
        }

        /// Cast to the underlying Rgba32 datatype
        operator rerun::datatypes::Rgba32() const {
            return rgba;
        }
    };
} // namespace rerun::components

namespace rerun {
    static_assert(sizeof(rerun::datatypes::Rgba32) == sizeof(components::Color));

    /// \private
    template <>
    struct Loggable<components::Color> {
        static constexpr const char Name[] = "rerun.components.Color";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return Loggable<rerun::datatypes::Rgba32>::arrow_datatype();
        }

        /// Serializes an array of `rerun::components::Color` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::Color* instances, size_t num_instances
        ) {
            return Loggable<rerun::datatypes::Rgba32>::to_arrow(&instances->rgba, num_instances);
        }
    };
} // namespace rerun
