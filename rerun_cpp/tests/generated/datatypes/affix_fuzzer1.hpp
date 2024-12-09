// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

#pragma once

#include "flattened_scalar.hpp"

#include <cstdint>
#include <memory>
#include <optional>
#include <rerun/collection.hpp>
#include <rerun/component_descriptor.hpp>
#include <rerun/result.hpp>
#include <string>

namespace arrow {
    class Array;
    class DataType;
    class StructBuilder;
} // namespace arrow

namespace rerun::datatypes {
    struct AffixFuzzer1 {
        std::optional<float> single_float_optional;

        std::string single_string_required;

        std::optional<std::string> single_string_optional;

        std::optional<rerun::Collection<float>> many_floats_optional;

        rerun::Collection<std::string> many_strings_required;

        std::optional<rerun::Collection<std::string>> many_strings_optional;

        float flattened_scalar;

        rerun::datatypes::FlattenedScalar almost_flattened_scalar;

        std::optional<bool> from_parent;

      public:
        AffixFuzzer1() = default;
    };
} // namespace rerun::datatypes

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<datatypes::AffixFuzzer1> {
        static constexpr ComponentDescriptor Descriptor = "rerun.testing.datatypes.AffixFuzzer1";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::datatypes::AffixFuzzer1` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const datatypes::AffixFuzzer1* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StructBuilder* builder, const datatypes::AffixFuzzer1* elements,
            size_t num_elements
        );
    };
} // namespace rerun
