// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs".

#pragma once

#include <cstdint>
#include <memory>
#include <rerun/half.hpp>
#include <rerun/result.hpp>
#include <vector>

namespace arrow {
    class DataType;
    class MemoryPool;
    class StructBuilder;
} // namespace arrow

namespace rerun {
    namespace datatypes {
        struct AffixFuzzer21 {
            rerun::half single_half;

            std::vector<rerun::half> many_halves;

          public:
            AffixFuzzer21() = default;

            /// Returns the arrow data type this type corresponds to.
            static const std::shared_ptr<arrow::DataType>& arrow_datatype();

            /// Creates a new array builder with an array of this type.
            static Result<std::shared_ptr<arrow::StructBuilder>> new_arrow_array_builder(
                arrow::MemoryPool* memory_pool
            );

            /// Fills an arrow array builder with an array of this type.
            static rerun::Error fill_arrow_array_builder(
                arrow::StructBuilder* builder, const AffixFuzzer21* elements, size_t num_elements
            );
        };
    } // namespace datatypes
} // namespace rerun
