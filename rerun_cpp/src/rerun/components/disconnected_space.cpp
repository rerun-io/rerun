// DO NOT EDIT!: This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs:53.
// Based on "crates/re_types/definitions/rerun/components/disconnected_space.fbs".

#include "disconnected_space.hpp"

#include "../arrow.hpp"

#include <arrow/builder.h>
#include <arrow/table.h>
#include <arrow/type_fwd.h>

namespace rerun {
    namespace components {
        const char *DisconnectedSpace::NAME = "rerun.disconnected_space";

        const std::shared_ptr<arrow::DataType> &DisconnectedSpace::arrow_datatype() {
            static const auto datatype = arrow::boolean();
            return datatype;
        }

        Result<std::shared_ptr<arrow::BooleanBuilder>> DisconnectedSpace::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(std::make_shared<arrow::BooleanBuilder>(memory_pool));
        }

        Error DisconnectedSpace::fill_arrow_array_builder(
            arrow::BooleanBuilder *builder, const DisconnectedSpace *elements, size_t num_elements
        ) {
            if (!builder) {
                return Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.");
            }
            if (!elements) {
                return Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Cannot serialize null pointer to arrow array."
                );
            }

            static_assert(sizeof(*elements) == sizeof(elements->is_disconnected));
            ARROW_RETURN_NOT_OK(builder->AppendValues(
                reinterpret_cast<const uint8_t *>(&elements->is_disconnected),
                static_cast<int64_t>(num_elements)
            ));

            return Error::ok();
        }

        Result<rerun::DataCell> DisconnectedSpace::to_data_cell(
            const DisconnectedSpace *instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool *pool = arrow::default_memory_pool();

            auto builder_result = DisconnectedSpace::new_arrow_array_builder(pool);
            RR_RETURN_NOT_OK(builder_result.error);
            auto builder = std::move(builder_result.value);
            if (instances && num_instances > 0) {
                RR_RETURN_NOT_OK(DisconnectedSpace::fill_arrow_array_builder(
                    builder.get(),
                    instances,
                    num_instances
                ));
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema(
                {arrow::field(DisconnectedSpace::NAME, DisconnectedSpace::arrow_datatype(), false)}
            );

            rerun::DataCell cell;
            cell.component_name = DisconnectedSpace::NAME;
            const auto ipc_result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
            RR_RETURN_NOT_OK(ipc_result.error);
            cell.buffer = std::move(ipc_result.value);

            return cell;
        }
    } // namespace components
} // namespace rerun
