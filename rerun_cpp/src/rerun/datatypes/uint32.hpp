// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/datatypes/uint32.fbs".

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
    class UInt32Type;
    using UInt32Builder = NumericBuilder<UInt32Type>;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: A 32bit unsigned integer.
    struct UInt32 {
        uint32_t value;

      public:
        UInt32() = default;

        UInt32(uint32_t value_) : value(value_) {}

        UInt32& operator=(uint32_t value_) {
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
    struct Loggable<datatypes::UInt32> {
        static constexpr const char Name[] = "rerun.datatypes.UInt32";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::UInt32` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::UInt32* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::UInt32Builder* builder, const datatypes::UInt32* elements, size_t num_elements
        );
    };
} // namespace rerun
