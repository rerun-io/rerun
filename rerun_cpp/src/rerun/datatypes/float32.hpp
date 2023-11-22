// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/datatypes/float32.fbs".

#pragma once

#include "../data_cell.hpp"
#include "../result.hpp"

#include <cstdint>
#include <memory>

namespace arrow {
    /// \private
    template <typename T>
    class NumericBuilder;

    class DataType;
    class FloatType;
    using FloatBuilder = NumericBuilder<FloatType>;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: A single-precision 32-bit IEEE 754 floating point number.
    struct Float32 {
        float value;

      public:
        Float32() = default;

        Float32(float value_) : value(value_) {}

        Float32& operator=(float value_) {
            value = value_;
            return *this;
        }
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::Float32> {
        static constexpr const char Name[] = "rerun.datatypes.Float32";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FloatBuilder* builder, const datatypes::Float32* elements, size_t num_elements
        );

        /// Creates a Rerun DataCell from an array of `rerun::datatypes::Float32` components.
        static Result<rerun::DataCell> to_arrow(
            const datatypes::Float32* instances, size_t num_instances
        );
    };
} // namespace rerun
