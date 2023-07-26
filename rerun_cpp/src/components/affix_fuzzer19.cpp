// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy.fbs"

#include "affix_fuzzer19.hpp"

#include "../datatypes/affix_fuzzer5.hpp"

#include <arrow/api.h>

namespace rr {
    namespace components {
        std::shared_ptr<arrow::DataType> AffixFuzzer19::to_arrow_datatype() {
            return rr::datatypes::AffixFuzzer5::to_arrow_datatype();
        }

        arrow::Result<std::shared_ptr<arrow::StructBuilder>> AffixFuzzer19::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(
                rr::datatypes::AffixFuzzer5::new_arrow_array_builder(memory_pool).ValueOrDie()
            );
        }

        arrow::Status AffixFuzzer19::fill_arrow_array_builder(
            arrow::StructBuilder *builder, const AffixFuzzer19 *elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            static_assert(sizeof(rr::datatypes::AffixFuzzer5) == sizeof(AffixFuzzer19));
            ARROW_RETURN_NOT_OK(rr::datatypes::AffixFuzzer5::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rr::datatypes::AffixFuzzer5 *>(elements),
                num_elements
            ));

            return arrow::Status::OK();
        }
    } // namespace components
} // namespace rr
