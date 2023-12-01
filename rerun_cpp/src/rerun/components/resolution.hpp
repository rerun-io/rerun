// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/resolution.fbs".

#pragma once

#include "../datatypes/vec2d.hpp"
#include "../result.hpp"

#include <array>
#include <cstdint>
#include <memory>

namespace arrow {
    class Array;
    class DataType;
    class FixedSizeListBuilder;
} // namespace arrow

namespace rerun::components {
    /// **Component**: Pixel resolution width & height, e.g. of a camera sensor.
    ///
    /// Typically in integer units, but for some use cases floating point may be used.
    struct Resolution {
        rerun::datatypes::Vec2D resolution;

      public:
        // Extensions to generated type defined in 'resolution_ext.cpp'

        static const Resolution IDENTITY;

        /// Construct resolution from width and height floats.
        Resolution(float width, float height) : resolution{width, height} {}

        /// Construct resolution from width and height integers.
        Resolution(int width, int height)
            : resolution{static_cast<float>(width), static_cast<float>(height)} {}

      public:
        Resolution() = default;

        Resolution(rerun::datatypes::Vec2D resolution_) : resolution(resolution_) {}

        Resolution& operator=(rerun::datatypes::Vec2D resolution_) {
            resolution = resolution_;
            return *this;
        }

        Resolution(std::array<float, 2> xy_) : resolution(xy_) {}

        Resolution& operator=(std::array<float, 2> xy_) {
            resolution = xy_;
            return *this;
        }

        /// Cast to the underlying Vec2D datatype
        operator rerun::datatypes::Vec2D() const {
            return resolution;
        }
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::Resolution> {
        static constexpr const char Name[] = "rerun.components.Resolution";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FixedSizeListBuilder* builder, const components::Resolution* elements,
            size_t num_elements
        );

        /// Serializes an array of `rerun::components::Resolution` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::Resolution* instances, size_t num_instances
        );
    };
} // namespace rerun
