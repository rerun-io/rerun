// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/affix_fuzzer3.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <utility>

namespace rr {
    namespace components {
        struct AffixFuzzer14 {
            rr::datatypes::AffixFuzzer3 single_required_union;

            /// Name of the component, used for serialization.
            static const char* NAME;

          public:
            AffixFuzzer14(rr::datatypes::AffixFuzzer3 single_required_union)
                : single_required_union(std::move(single_required_union)) {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::DenseUnionBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::DenseUnionBuilder* builder, const AffixFuzzer14* elements,
                size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of AffixFuzzer14 components.
            static arrow::Result<rr::DataCell> to_data_cell(
                const AffixFuzzer14* components, size_t num_components
            );
        };
    } // namespace components
} // namespace rr
