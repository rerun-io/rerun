// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/blob.fbs".

#include "blob.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun {
    namespace components {
        const char Blob::NAME[] = "rerun.components.Blob";

        const std::shared_ptr<arrow::DataType>& Blob::arrow_datatype() {
            static const auto datatype = arrow::list(arrow::field("item", arrow::uint8(), false));
            return datatype;
        }

        Result<std::shared_ptr<arrow::ListBuilder>> Blob::new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        ) {
            if (memory_pool == nullptr) {
                return rerun::Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(std::make_shared<arrow::ListBuilder>(
                memory_pool,
                std::make_shared<arrow::UInt8Builder>(memory_pool)
            ));
        }

        rerun::Error Blob::fill_arrow_array_builder(
            arrow::ListBuilder* builder, const Blob* elements, size_t num_elements
        ) {
            if (builder == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Passed array builder is null."
                );
            }
            if (elements == nullptr) {
                return rerun::Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Cannot serialize null pointer to arrow array."
                );
            }

            auto value_builder = static_cast<arrow::UInt8Builder*>(builder->value_builder());
            ARROW_RETURN_NOT_OK(builder->Reserve(static_cast<int64_t>(num_elements)));
            ARROW_RETURN_NOT_OK(value_builder->Reserve(static_cast<int64_t>(num_elements * 2)));

            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto& element = elements[elem_idx];
                ARROW_RETURN_NOT_OK(builder->Append());
                ARROW_RETURN_NOT_OK(value_builder->AppendValues(
                    element.data.data(),
                    static_cast<int64_t>(element.data.size()),
                    nullptr
                ));
            }

            return Error::ok();
        }

        Result<rerun::DataCell> Blob::to_data_cell(const Blob* instances, size_t num_instances) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool* pool = arrow::default_memory_pool();

            auto builder_result = Blob::new_arrow_array_builder(pool);
            RR_RETURN_NOT_OK(builder_result.error);
            auto builder = std::move(builder_result.value);
            if (instances && num_instances > 0) {
                RR_RETURN_NOT_OK(
                    Blob::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            return rerun::DataCell::create(Blob::NAME, Blob::arrow_datatype(), std::move(array));
        }
    } // namespace components
} // namespace rerun
