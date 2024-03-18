// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#pragma once

#include "../datatypes/affix_fuzzer22.hpp"

#include <array>
#include <cstdint>
#include <memory>
#include <optional>
#include <rerun/result.hpp>

namespace arrow {
    class Array;
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::components {
    struct AffixFuzzer22 {
        std::optional<rerun::datatypes::AffixFuzzer22> nullable_nested_array;

      public:
        AffixFuzzer22() = default;

        AffixFuzzer22(std::optional<rerun::datatypes::AffixFuzzer22> nullable_nested_array_)
            : nullable_nested_array(nullable_nested_array_) {}

        AffixFuzzer22& operator=(
            std::optional<rerun::datatypes::AffixFuzzer22> nullable_nested_array_
        ) {
            nullable_nested_array = nullable_nested_array_;
            return *this;
        }

        AffixFuzzer22(std::array<uint8_t, 4> fixed_sized_native_)
            : nullable_nested_array(fixed_sized_native_) {}

        AffixFuzzer22& operator=(std::array<uint8_t, 4> fixed_sized_native_) {
            nullable_nested_array = fixed_sized_native_;
            return *this;
        }

        /// Cast to the underlying AffixFuzzer22 datatype
        operator std::optional<rerun::datatypes::AffixFuzzer22>() const {
            return nullable_nested_array;
        }
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::AffixFuzzer22> {
        static constexpr const char Name[] = "rerun.testing.components.AffixFuzzer22";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::components::AffixFuzzer22` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::AffixFuzzer22* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const components::AffixFuzzer22* elements,
            size_t num_elements
        );
    };
} // namespace rerun
