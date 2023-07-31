// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#pragma once

#include "../data_cell.hpp"

#include <arrow/type_fwd.h>
#include <cstdint>
#include <string>
#include <utility>
#include <vector>

namespace rr {
    namespace components {
        struct AffixFuzzer12 {
            std::vector<std::string> many_strings_required;

            /// Name of the component, used for serialization.
            static const char* NAME;

          public:
            AffixFuzzer12(std::vector<std::string> many_strings_required)
                : many_strings_required(std::move(many_strings_required)) {}

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& to_arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::ListBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static arrow::Status fill_arrow_array_builder(
                arrow::ListBuilder* builder, const AffixFuzzer12* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of AffixFuzzer12 components.
            static arrow::Result<rr::DataCell> to_data_cell(
                const AffixFuzzer12* components, size_t num_components
            );
        };
    } // namespace components
} // namespace rr
