// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/scalar.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    template <typename T>
    class NumericBuilder;

    class DataType;
    class DoubleType;
    class MemoryPool;
    using DoubleBuilder = NumericBuilder<DoubleType>;
} // namespace arrow

namespace rerun {
    namespace components {
        /// **Component**: A double-precision scalar.
        ///
        /// Used for time series plots.
        struct Scalar {
            double value;

            /// Name of the component, used for serialization.
            static const char NAME[];

          public:
            Scalar() = default;

            Scalar(double value_) : value(value_) {}

            Scalar& operator=(double value_) {
                value = value_;
                return *this;
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::DoubleBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::DoubleBuilder* builder, const Scalar* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of Scalar components.
            static Result<rerun::DataCell> to_data_cell(
                const Scalar* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
