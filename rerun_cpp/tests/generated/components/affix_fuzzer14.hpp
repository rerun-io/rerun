// DO NOT EDIT!: This file was autogenerated by re_types_builder in
// crates/re_types_builder/src/codegen/cpp/mod.rs:54 Based on
// "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#pragma once

#include "../datatypes/affix_fuzzer3.hpp"

#include <cstdint>
#include <memory>
#include <rerun/data_cell.hpp>
#include <rerun/result.hpp>
#include <utility>

namespace arrow {
    class DataType;
    class DenseUnionBuilder;
    class MemoryPool;
} // namespace arrow

namespace rerun {
    namespace components {
        struct AffixFuzzer14 {
            rerun::datatypes::AffixFuzzer3 single_required_union;

            /// Name of the component, used for serialization.
            static const char* NAME;

          public:
            AffixFuzzer14() = default;

            AffixFuzzer14(rerun::datatypes::AffixFuzzer3 _single_required_union)
                : single_required_union(std::move(_single_required_union)) {}

            AffixFuzzer14& operator=(rerun::datatypes::AffixFuzzer3 _single_required_union) {
                single_required_union = std::move(_single_required_union);
                return *this;
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::DenseUnionBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static Error fill_arrow_array_builder(
                arrow::DenseUnionBuilder* builder, const AffixFuzzer14* elements,
                size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of AffixFuzzer14 components.
            static Result<rerun::DataCell> to_data_cell(
                const AffixFuzzer14* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
