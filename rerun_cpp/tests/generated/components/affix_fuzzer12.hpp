// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#pragma once

#include <cstdint>
#include <memory>
#include <rerun/collection.hpp>
#include <rerun/result.hpp>
#include <string>
#include <utility>

namespace arrow {
    class Array;
    class DataType;
    class ListBuilder;
} // namespace arrow

namespace rerun::components {
    struct AffixFuzzer12 {
        rerun::Collection<std::string> many_strings_required;

      public:
        AffixFuzzer12() = default;

        AffixFuzzer12(rerun::Collection<std::string> many_strings_required_)
            : many_strings_required(std::move(many_strings_required_)) {}

        AffixFuzzer12& operator=(rerun::Collection<std::string> many_strings_required_) {
            many_strings_required = std::move(many_strings_required_);
            return *this;
        }
    };
} // namespace rerun::components

namespace rerun {
    template <typename T>
    struct Loggable;

    /// \private
    template <>
    struct Loggable<components::AffixFuzzer12> {
        static constexpr const char Name[] = "rerun.testing.components.AffixFuzzer12";

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Serializes an array of `rerun::components::AffixFuzzer12` into an arrow array.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::AffixFuzzer12* instances, size_t num_instances
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::ListBuilder* builder, const components::AffixFuzzer12* elements,
            size_t num_elements
        );
    };
} // namespace rerun
