// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#pragma once

#include <cstdint>
#include <memory>
#include <rerun/collection.hpp>
#include <rerun/data_cell.hpp>
#include <rerun/result.hpp>
#include <string>
#include <utility>

namespace arrow {
    class DataType;
    class ListBuilder;
} // namespace arrow

namespace rerun::components {
    struct AffixFuzzer12 {
        rerun::Collection<std::string> many_strings_required;

        /// Name of the component, used for serialization.
        static const char NAME[];

      public:
        AffixFuzzer12() = default;

        AffixFuzzer12(rerun::Collection<std::string> many_strings_required_)
            : many_strings_required(std::move(many_strings_required_)) {}

        AffixFuzzer12& operator=(rerun::Collection<std::string> many_strings_required_) {
            many_strings_required = std::move(many_strings_required_);
            return *this;
        }

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::ListBuilder* builder, const AffixFuzzer12* elements, size_t num_elements
        );

        /// Creates a Rerun DataCell from an array of AffixFuzzer12 components.
        static Result<rerun::DataCell> to_data_cell(
            const AffixFuzzer12* instances, size_t num_instances
        );
    };
} // namespace rerun::components
