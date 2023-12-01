// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/scalar.fbs".

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
    class DoubleType;
    using DoubleBuilder = NumericBuilder<DoubleType>;
} // namespace arrow

namespace rerun::components {
    /// **Component**: A double-precision scalar.
    ///
    /// Used for time series plots.
    struct Scalar {
        double value;

      public:
        Scalar() = default;

        Scalar(double value_) : value(value_) {}

        Scalar& operator=(double value_) {
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
    struct Loggable<components::Scalar> {
        static constexpr const char Name[] = "rerun.components.Scalar";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::DoubleBuilder* builder, const components::Scalar* elements, size_t num_elements
        );

        /// Serializes an array of `rerun::components::Scalar` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::Scalar* instances, size_t num_instances
        );
    };
} // namespace rerun
