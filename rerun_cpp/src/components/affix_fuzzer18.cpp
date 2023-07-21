// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include "affix_fuzzer18.hpp"

#include "../datatypes/affix_fuzzer4.hpp"

#include <arrow/api.h>

namespace rr {
    namespace components {
        std::shared_ptr<arrow::DataType> AffixFuzzer18::to_arrow_datatype() {
            return arrow::list(arrow::field(
                "item", rr::datatypes::AffixFuzzer4::to_arrow_datatype(), true, nullptr));
        }

        arrow::Result<std::shared_ptr<arrow::ArrayBuilder>> AffixFuzzer18::to_arrow(
            arrow::MemoryPool* memory_pool, const AffixFuzzer18* elements, size_t num_elements) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            auto builder = std::make_shared<arrow::ListBuilder>(memory_pool);
            return builder;
        }
    } // namespace components
} // namespace rr
