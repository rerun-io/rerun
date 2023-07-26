// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include "affix_fuzzer6.hpp"

#include "../datatypes/affix_fuzzer1.hpp"

#include <arrow/api.h>

namespace rr {
    namespace components {
        std::shared_ptr<arrow::DataType> AffixFuzzer6::to_arrow_datatype() {
            return rr::datatypes::AffixFuzzer1::to_arrow_datatype();
        }

        arrow::Result<std::shared_ptr<arrow::StructBuilder>> AffixFuzzer6::new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(
                rr::datatypes::AffixFuzzer1::new_arrow_array_builder(memory_pool).ValueOrDie()
            );
        }

        arrow::Status AffixFuzzer6::fill_arrow_array_builder(
            arrow::StructBuilder* builder, const AffixFuzzer6* elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            return arrow::Status::NotImplemented(("TODO(andreas) Handle nullable extensions"));

            return arrow::Status::OK();
        }
    } // namespace components
} // namespace rr
