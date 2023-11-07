// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs".

#pragma once

#include "../datatypes/affix_fuzzer20.hpp"

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
        struct AffixFuzzer20 {
            rerun::datatypes::AffixFuzzer20 nested_transparent;

            /// Name of the component, used for serialization.
            static const char NAME[];

          public:
            AffixFuzzer20() = default;

            AffixFuzzer20(rerun::datatypes::AffixFuzzer20 nested_transparent_)
                : nested_transparent(std::move(nested_transparent_)) {}

            AffixFuzzer20& operator=(rerun::datatypes::AffixFuzzer20 nested_transparent_) {
                nested_transparent = std::move(nested_transparent_);
                return *this;
            }

            /// Cast to the underlying AffixFuzzer20 datatype
            operator rerun::datatypes::AffixFuzzer20() const {
                return nested_transparent;
            }

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static rerun::Error fill_arrow_array_builder(
                arrow::StructBuilder* builder, const AffixFuzzer20* elements, size_t num_elements
            );

            /// Creates a Rerun DataCell from an array of AffixFuzzer20 components.
            static Result<rerun::DataCell> to_data_cell(
                const AffixFuzzer20* instances, size_t num_instances
            );
        };
    } // namespace components
} // namespace rerun
