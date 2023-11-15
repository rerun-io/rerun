// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#pragma once

#include <cstdint>
#include <memory>
#include <rerun/data_cell.hpp>
#include <rerun/result.hpp>
#include <string>
#include <utility>

namespace arrow {
    class DataType;
    class MemoryPool;
    class StringBuilder;
} // namespace arrow

namespace rerun::components {
    struct AffixFuzzer9 {
        std::string single_string_required;

        /// Name of the component, used for serialization.
        static const char NAME[];

      public:
        AffixFuzzer9() = default;

        AffixFuzzer9(std::string single_string_required_)
            : single_string_required(std::move(single_string_required_)) {}

        AffixFuzzer9& operator=(std::string single_string_required_) {
            single_string_required = std::move(single_string_required_);
            return *this;
        }

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype();

        /// Creates a new array builder with an array of this type.
        static Result<std::shared_ptr<arrow::StringBuilder>> new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        );

        /// Fills an arrow array builder with an array of this type.
        static rerun::Error fill_arrow_array_builder(
            arrow::StringBuilder* builder, const AffixFuzzer9* elements, size_t num_elements
        );

        /// Creates a Rerun DataCell from an array of AffixFuzzer9 components.
        static Result<rerun::DataCell> to_data_cell(
            const AffixFuzzer9* instances, size_t num_instances
        );
    };
} // namespace rerun::components
