// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:54.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#pragma once

#include "../datatypes/affix_fuzzer1.hpp"

#include <cstdint>
#include <memory>
#include <rerun/data_cell.hpp>
#include <rerun/result.hpp>
#include <utility>

namespace arrow {
    class DataType;
    class MemoryPool;
    class StructBuilder;
} // namespace arrow

namespace rerun {
    namespace components {
        struct AffixFuzzer3 {
            rerun::datatypes::AffixFuzzer1 single_required;

            /// Name of the component, used for serialization.
            static const char NAME[];

          public:
            AffixFuzzer3() = default;

            AffixFuzzer3(rerun::datatypes::AffixFuzzer1 _single_required)
                : single_required(std::move(_single_required)) {}

            AffixFuzzer3& operator=(rerun::datatypes::AffixFuzzer1 _single_required) {
                single_required = std::move(_single_required);
                return *this;
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::StructBuilder* builder, const AffixFuzzer3* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of AffixFuzzer3 components.
            static Result<rerun::DataCell> to_data_cell(
                const AffixFuzzer3* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
