// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/testing/components/enum_test.fbs".

#pragma once

#include <cstdint>
#include <memory>
#include <rerun/result.hpp>

namespace arrow {
    /// \private
    template <typename T>
    class NumericBuilder;

    class Array;
    class DataType;
    class UInt8Type;
    using UInt8Builder = NumericBuilder<UInt8Type>;
} // namespace arrow

namespace rerun::datatypes {
    /// **Datatype**: A test of the enum type.
    enum class EnumTest : uint8_t {

        /// Great film.
        Up = 1,

        /// Feeling blue.
        Down = 2,

        /// Correct.
        Right = 3,

        /// It's what's remaining.
        Left = 4,

        /// It's the only way to go.
        Forward = 5,

        /// Baby's got it.
        Back = 6,
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::EnumTest> {
        static constexpr const char Name[] = "rerun.testing.datatypes.EnumTest";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::EnumTest` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::EnumTest* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::UInt8Builder* builder, const datatypes::EnumTest* elements, size_t num_elements
        );
    };
} // namespace rerun
