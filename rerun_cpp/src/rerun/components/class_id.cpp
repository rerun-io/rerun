// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/class_id.fbs".

#include "class_id.hpp"

#include "../arrow.hpp"
#include "../datatypes/class_id.hpp"

#include <arrow/builder.h>
#include <arrow/table.h>
#include <arrow/type_fwd.h>

namespace rerun {
    namespace components {
        const char ClassId::NAME[] = "rerun.components.ClassId";

        const std::shared_ptr<arrow::DataType> &ClassId::arrow_datatype() {
            static const auto datatype = rerun::datatypes::ClassId::arrow_datatype();
            return datatype;
        }

        Result<std::shared_ptr<arrow::UInt16Builder>> ClassId::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(rerun::datatypes::ClassId::new_arrow_array_builder(memory_pool).value);
        }

        Error ClassId::fill_arrow_array_builder(
            arrow::UInt16Builder *builder, const ClassId *elements, size_t num_elements
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

            static_assert(sizeof(rerun::datatypes::ClassId) == sizeof(ClassId));
            RR_RETURN_NOT_OK(rerun::datatypes::ClassId::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rerun::datatypes::ClassId *>(elements),
                num_elements
            ));

            return Error::ok();
        }

        Result<rerun::DataCell> ClassId::to_data_cell(
            const ClassId *instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool *pool = arrow::default_memory_pool();

            auto builder_result = ClassId::new_arrow_array_builder(pool);
            RR_RETURN_NOT_OK(builder_result.error);
            auto builder = std::move(builder_result.value);
            if (instances && num_instances > 0) {
                RR_RETURN_NOT_OK(
                    ClassId::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema =
                arrow::schema({arrow::field(ClassId::NAME, ClassId::arrow_datatype(), false)});

            rerun::DataCell cell;
            cell.component_name = ClassId::NAME;
            const auto ipc_result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
            RR_RETURN_NOT_OK(ipc_result.error);
            cell.buffer = std::move(ipc_result.value);

            return cell;
        }
    } // namespace components
} // namespace rerun
