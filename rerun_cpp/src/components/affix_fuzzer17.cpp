// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include "affix_fuzzer17.hpp"

#include "../datatypes/affix_fuzzer3.hpp"

#include <arrow/api.h>

namespace rr {
    namespace components {
        std::shared_ptr<arrow::DataType> AffixFuzzer17::to_arrow_datatype() {
            return arrow::list(arrow::field(
                "item",
                rr::datatypes::AffixFuzzer3::to_arrow_datatype(),
                true,
                nullptr
            ));
        }

        arrow::Result<std::shared_ptr<arrow::ListBuilder>> AffixFuzzer17::new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::ListBuilder>(
                memory_pool,
                rr::datatypes::AffixFuzzer3::new_arrow_array_builder(memory_pool).ValueOrDie()
            ));
        }

        arrow::Status AffixFuzzer17::fill_arrow_array_builder(
            arrow::ListBuilder* builder, const AffixFuzzer17* elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            return arrow::Status::NotImplemented(
                "TODO(andreas): custom data types in lists/fixedsizelist are not yet implemented"
            );

            return arrow::Status::OK();
        }
    } // namespace components
} // namespace rr
