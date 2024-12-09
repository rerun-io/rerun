// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

#pragma once

#include <cstdint>
#include <memory>
#include <optional>
#include <rerun/component_descriptor.hpp>
#include <rerun/result.hpp>

namespace arrow {
    /// \private
    template <typename T>
    class NumericBuilder;

    class Array;
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
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::AffixFuzzer2> {
        static constexpr ComponentDescriptor Descriptor = "rerun.testing.datatypes.AffixFuzzer2";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::AffixFuzzer2` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::AffixFuzzer2* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::FloatBuilder* builder, const datatypes::AffixFuzzer2* elements,
            size_t num_elements
        );
    };
} // namespace rerun
