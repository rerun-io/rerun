// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#pragma once

#include "../data_cell.hpp"
#include "../datatypes/affix_fuzzer1.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <utility>

namespace rr {
    namespace components {
        struct AffixFuzzer2 {
            rr::datatypes::AffixFuzzer1 single_required;

            /// Name of the component, used for serialization.
            static const char* NAME;

          public:
            AffixFuzzer2(rr::datatypes::AffixFuzzer1 single_required)
                : single_required(std::move(single_required)) {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::StructBuilder* builder, const AffixFuzzer2* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of AffixFuzzer2 components.
            static arrow::Result<rr::DataCell> to_data_cell(
                const AffixFuzzer2* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rr
