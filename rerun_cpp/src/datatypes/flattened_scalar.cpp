// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/datatypes/fuzzy.fbs"

#include "flattened_scalar.hpp"

#include <arrow/api.h>

namespace rr {
    namespace datatypes {
        std::shared_ptr<arrow::DataType> FlattenedScalar::to_arrow_datatype() {
            return arrow::struct_({
                arrow::field("value", arrow::float32(), false, nullptr),
            });
        }

        arrow::Result<std::shared_ptr<arrow::ArrayBuilder>> FlattenedScalar::to_arrow(
            arrow::MemoryPool* memory_pool, const FlattenedScalar* elements, size_t num_elements) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            auto datatype = FlattenedScalar::to_arrow_datatype();
            let builder =
                std::make_shared<arrow::FixedSizeBinaryBuilder>(datatype, memory_pool, {},
                                                                // TODO(#2647): code-gen for C++
                );
            return builder;
        }
    } // namespace datatypes
} // namespace rr
