// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

#pragma once

#include <cstdint>
#include <memory>
#include <optional>
#include <rerun/result.hpp>

namespace arrow {
    /// \private
    template <typename T>
    class NumericBuilder;

    class DataType;
    class FloatType;
    using FloatBuilder = NumericBuilder<FloatType>;
} // namespace arrow

namespace rerun::datatypes {
    struct AffixFuzzer2 {
        std::optional<float> single_float_optional;

      public:
        AffixFuzzer2() = default;

        AffixFuzzer2(std::optional<float> single_float_optional_)
            : single_float_optional(single_float_optional_) {}

        AffixFuzzer2& operator=(std::optional<float> single_float_optional_) {
            single_float_optional = single_float_optional_;
            return *this;
        }

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FloatBuilder* builder, const AffixFuzzer2* elements, size_t num_elements
        );
    };
} // namespace rerun::datatypes
