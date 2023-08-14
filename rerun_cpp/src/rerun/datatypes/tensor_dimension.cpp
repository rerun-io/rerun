// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/datatypes/tensor_dimension.fbs"

#include "tensor_dimension.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace datatypes {
        const std::shared_ptr<arrow::DataType> &TensorDimension::to_arrow_datatype() {
            static const auto datatype = arrow::struct_({
                arrow::field("size", arrow::int64(), false),
                arrow::field("name", arrow::utf8(), true),
            });
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::StructBuilder>>
            TensorDimension::new_arrow_array_builder(arrow::MemoryPool *memory_pool) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::StructBuilder>(
                to_arrow_datatype(),
                memory_pool,
                std::vector<std::shared_ptr<arrow::ArrayBuilder>>({
                    std::make_shared<arrow::Int64Builder>(memory_pool),
                    std::make_shared<arrow::StringBuilder>(memory_pool),
                })
            ));
        }

        arrow::Status TensorDimension::fill_arrow_array_builder(
            arrow::StructBuilder *builder, const TensorDimension *elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            {
                auto field_builder = static_cast<arrow::Int64Builder *>(builder->field_builder(0));
                ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
                for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                    ARROW_RETURN_NOT_OK(field_builder->Append(elements[elem_idx].size));
                }
            }
            {
                auto field_builder = static_cast<arrow::StringBuilder *>(builder->field_builder(1));
                ARROW_RETURN_NOT_OK(field_builder->Reserve(static_cast<int64_t>(num_elements)));
                for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                    const auto &element = elements[elem_idx];
                    if (element.name.has_value()) {
                        ARROW_RETURN_NOT_OK(field_builder->Append(element.name.value()));
                    } else {
                        ARROW_RETURN_NOT_OK(field_builder->AppendNull());
                    }
                }
            }
            ARROW_RETURN_NOT_OK(builder->AppendValues(static_cast<int64_t>(num_elements), nullptr));

            return arrow::Status::OK();
        }
    } // namespace datatypes
} // namespace rerun
