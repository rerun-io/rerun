// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/quaternion.fbs"

#include "quaternion.hpp"

#include <arrow/api.h>

namespace rr {
    namespace datatypes {
        std::shared_ptr<arrow::DataType> Quaternion::to_arrow_datatype() {
            return arrow::fixed_size_list(arrow::field("item", arrow::float32(), false, nullptr),
                                          4);
        }

        arrow::Result<std::shared_ptr<arrow::ArrayBuilder>> Quaternion::to_arrow(
            arrow::MemoryPool* memory_pool, const Quaternion* elements, size_t num_elements) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            auto builder = std::make_shared<arrow::FixedSizeListBuilder>(memory_pool);
            return builder;
        }
    } // namespace datatypes
} // namespace rr
