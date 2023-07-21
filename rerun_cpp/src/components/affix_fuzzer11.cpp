// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include "affix_fuzzer11.hpp"

#include <arrow/api.h>

namespace rr {
    namespace components {
        std::shared_ptr<arrow::DataType> AffixFuzzer11::to_arrow_datatype() {
            return arrow::list(arrow::field("item", arrow::float32(), true, nullptr));
        }

        arrow::Result<std::shared_ptr<arrow::ArrayBuilder>> AffixFuzzer11::to_arrow(
            arrow::MemoryPool* memory_pool, const AffixFuzzer11* elements, size_t num_elements) {
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
