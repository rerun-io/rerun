// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#pragma once

#include "../datatypes/affix_fuzzer3.hpp"

#include <arrow/result.h>
#include <cstdint>
#include <memory>
#include <optional>
#include <utility>
#include <vector>

namespace arrow {
    class ArrayBuilder;
    class DataType;
    class MemoryPool;
} // namespace arrow

namespace rr {
    namespace components {
        struct AffixFuzzer17 {
            std::optional<std::vector<rr::datatypes::AffixFuzzer3>> many_optional_unions;

          public:
            AffixFuzzer17(
                std::optional<std::vector<rr::datatypes::AffixFuzzer3>> many_optional_unions)
                : many_optional_unions(std::move(many_optional_unions)) {}

            /// Returns the arrow data type this type corresponds to.
            static std::shared_ptr<arrow::DataType> to_arrow_datatype();

            /// Fills out an arrow array builder with an array of this type.
            static arrow::Result<std::shared_ptr<arrow::ArrayBuilder>> to_arrow(
                arrow::MemoryPool* memory_pool, const AffixFuzzer17* elements, size_t num_elements);
        };
    } // namespace components
} // namespace rr
