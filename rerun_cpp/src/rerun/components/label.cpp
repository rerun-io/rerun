// DO NOT EDIT!: This file was autogenerated by re_types_builder in
// crates/re_types_builder/src/codegen/cpp/mod.rs:54 Based on
// "crates/re_types/definitions/rerun/components/label.fbs"

#include "label.hpp"

#include "../arrow.hpp"
#include "../datatypes/label.hpp"

#include <arrow/builder.h>
#include <arrow/table.h>
#include <arrow/type_fwd.h>

namespace rerun {
    namespace components {
        const char *Label::NAME = "rerun.label";

        const std::shared_ptr<arrow::DataType> &Label::arrow_datatype() {
            static const auto datatype = rerun::datatypes::Label::arrow_datatype();
            return datatype;
        }

        Result<std::shared_ptr<arrow::StringBuilder>> Label::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(rerun::datatypes::Label::new_arrow_array_builder(memory_pool).value);
        }

        Error Label::fill_arrow_array_builder(
            arrow::StringBuilder *builder, const Label *elements, size_t num_elements
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

            static_assert(sizeof(rerun::datatypes::Label) == sizeof(Label));
            RR_RETURN_NOT_OK(rerun::datatypes::Label::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rerun::datatypes::Label *>(elements),
                num_elements
            ));

            return Error::ok();
        }

        Result<rerun::DataCell> Label::to_data_cell(const Label *instances, size_t num_instances) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool *pool = arrow::default_memory_pool();

            auto builder_result = Label::new_arrow_array_builder(pool);
            RR_RETURN_NOT_OK(builder_result.error);
            auto builder = std::move(builder_result.value);
            if (instances && num_instances > 0) {
                RR_RETURN_NOT_OK(
                    Label::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema =
                arrow::schema({arrow::field(Label::NAME, Label::arrow_datatype(), false)});

            rerun::DataCell cell;
            cell.component_name = Label::NAME;
            const auto ipc_result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
            RR_RETURN_NOT_OK(ipc_result.error);
            cell.buffer = std::move(ipc_result.value);

            return cell;
        }
    } // namespace components
} // namespace rerun
