// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/marker_size.fbs".

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
    /// **Component**: Radius of a marker of a point in e.g. a 2D plot, measured in UI points.
    struct MarkerSize {
        float value;

      public:
        MarkerSize() = default;

        MarkerSize(float value_) : value(value_) {}

        MarkerSize& operator=(float value_) {
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
    struct Loggable<components::MarkerSize> {
        static constexpr const char Name[] = "rerun.components.MarkerSize";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::components::MarkerSize` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::MarkerSize* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FloatBuilder* builder, const components::MarkerSize* elements,
            size_t num_elements
        );
    };
} // namespace rerun
