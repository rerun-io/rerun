// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/depth_meter.fbs".

#pragma once

#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    /// \private
    template <typename T>
    class NumericBuilder;

    class Array;
    class DataType;
    class FloatType;
    using FloatBuilder = NumericBuilder<FloatType>;
} // namespace arrow

namespace rerun::components {
    /// **Component**: A component indicating how long a meter is, expressed in native units.
    struct DepthMeter {
        float value;

      public:
        DepthMeter() = default;

        DepthMeter(float value_) : value(value_) {}

        DepthMeter& operator=(float value_) {
            value = value_;
            return *this;
        }
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::DepthMeter> {
        static constexpr const char Name[] = "rerun.components.DepthMeter";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::components::DepthMeter` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::DepthMeter* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FloatBuilder* builder, const components::DepthMeter* elements,
            size_t num_elements
        );
    };
} // namespace rerun
