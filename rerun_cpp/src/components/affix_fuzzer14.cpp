// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include "affix_fuzzer14.hpp"

#include "../datatypes/affix_fuzzer3.hpp"

#include <arrow/api.h>

namespace rr {
    namespace components {
        std::shared_ptr<arrow::DataType> AffixFuzzer14::to_arrow_datatype() {
            return rr::datatypes::AffixFuzzer3::to_arrow_datatype();
        }

        arrow::Result<std::shared_ptr<arrow::ArrayBuilder>> AffixFuzzer14::to_arrow(
            arrow::MemoryPool *memory_pool, const AffixFuzzer14 *elements, size_t num_elements) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            static_assert(sizeof(AffixFuzzer14) == sizeof(rr::datatypes::AffixFuzzer3),
                          "Expected fully transparent type.");
            auto builder = rr::datatypes::AffixFuzzer3::to_arrow(
                memory_pool,
                reinterpret_cast<const rr::datatypes::AffixFuzzer3 *>(elements),
                num_elements);
            return builder;
        }
    } // namespace components
} // namespace rr
