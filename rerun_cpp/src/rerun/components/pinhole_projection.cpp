// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/pinhole_projection.fbs".

#include "pinhole_projection.hpp"

#include "../datatypes/mat3x3.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun {
    namespace components {
        const char PinholeProjection::NAME[] = "rerun.components.PinholeProjection";

        const std::shared_ptr<arrow::DataType>& PinholeProjection::arrow_datatype() {
            static const auto datatype = rerun::datatypes::Mat3x3::arrow_datatype();
            return datatype;
        }

        Result<std::shared_ptr<arrow::FixedSizeListBuilder>>
            PinholeProjection::new_arrow_array_builder(arrow::MemoryPool* memory_pool) {
            if (memory_pool == nullptr) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(rerun::datatypes::Mat3x3::new_arrow_array_builder(memory_pool).value);
        }

        Error PinholeProjection::fill_arrow_array_builder(
            arrow::FixedSizeListBuilder* builder, const PinholeProjection* elements,
            size_t num_elements
        ) {
            if (builder == nullptr) {
                return Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.");
            }
            if (elements == nullptr) {
                return Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Cannot serialize null pointer to arrow array."
                );
            }

            static_assert(sizeof(rerun::datatypes::Mat3x3) == sizeof(PinholeProjection));
            RR_RETURN_NOT_OK(rerun::datatypes::Mat3x3::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rerun::datatypes::Mat3x3*>(elements),
                num_elements
            ));

            return Error::ok();
        }

        Result<rerun::DataCell> PinholeProjection::to_data_cell(
            const PinholeProjection* instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool* pool = arrow::default_memory_pool();

            auto builder_result = PinholeProjection::new_arrow_array_builder(pool);
            RR_RETURN_NOT_OK(builder_result.error);
            auto builder = std::move(builder_result.value);
            if (instances && num_instances > 0) {
                RR_RETURN_NOT_OK(PinholeProjection::fill_arrow_array_builder(
                    builder.get(),
                    instances,
                    num_instances
                ));
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            return rerun::DataCell::create(
                PinholeProjection::NAME,
                PinholeProjection::arrow_datatype(),
                std::move(array)
            );
        }
    } // namespace components
} // namespace rerun
