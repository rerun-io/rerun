// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#pragma once

#include <cstdint>
#include <memory>
#include <rerun/data_cell.hpp>
#include <rerun/result.hpp>
#include <string>
#include <utility>
#include <vector>

namespace arrow {
    class DataType;
    class ListBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun::components {
    struct AffixFuzzer12 {
        std::vector<std::string> many_strings_required;

        /// Name of the component, used for serialization.
        static const char NAME[];

      public:
        AffixFuzzer12() = default;

        AffixFuzzer12(std::vector<std::string> many_strings_required_)
            : many_strings_required(std::move(many_strings_required_)) {}

        AffixFuzzer12& operator=(std::vector<std::string> many_strings_required_) {
            many_strings_required = std::move(many_strings_required_);
            return *this;
        }

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Creates a new array builder with an array of this type.
        static Result<std::shared_ptr<arrow::ListBuilder>> new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        );

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
