// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/tensor_id.fbs"

#include "tensor_id.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace datatypes {
        const std::shared_ptr<arrow::DataType> &TensorId::to_arrow_datatype() {
            static const auto datatype =
                arrow::fixed_size_list(arrow::field("item", arrow::uint8(), false), 16);
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::FixedSizeListBuilder>>
            TensorId::new_arrow_array_builder(arrow::MemoryPool *memory_pool) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::FixedSizeListBuilder>(
                memory_pool,
                std::make_shared<arrow::UInt8Builder>(memory_pool),
                16
            ));
        }

        arrow::Status TensorId::fill_arrow_array_builder(
            arrow::FixedSizeListBuilder *builder, const TensorId *elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            auto value_builder = static_cast<arrow::UInt8Builder *>(builder->value_builder());

            ARROW_RETURN_NOT_OK(builder->AppendValues(static_cast<int64_t>(num_elements)));
            static_assert(sizeof(elements[0].id) == sizeof(elements[0]));
            ARROW_RETURN_NOT_OK(value_builder->AppendValues(
                elements[0].id,
                static_cast<int64_t>(num_elements * 16),
                nullptr
            ));

            return arrow::Status::OK();
        }
    } // namespace datatypes
} // namespace rerun
