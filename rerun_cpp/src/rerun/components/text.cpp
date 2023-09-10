// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/text.fbs".

#include "text.hpp"

#include "../arrow.hpp"
#include "../datatypes/utf8.hpp"

#include <arrow/builder.h>
#include <arrow/table.h>
#include <arrow/type_fwd.h>

namespace rerun {
    namespace components {
        const char Text::NAME[] = "rerun.label";

        const std::shared_ptr<arrow::DataType> &Text::arrow_datatype() {
            static const auto datatype = rerun::datatypes::Utf8::arrow_datatype();
            return datatype;
        }

        Result<std::shared_ptr<arrow::StringBuilder>> Text::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(rerun::datatypes::Utf8::new_arrow_array_builder(memory_pool).value);
        }

        Error Text::fill_arrow_array_builder(
            arrow::StringBuilder *builder, const Text *elements, size_t num_elements
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

            static_assert(sizeof(rerun::datatypes::Utf8) == sizeof(Text));
            RR_RETURN_NOT_OK(rerun::datatypes::Utf8::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rerun::datatypes::Utf8 *>(elements),
                num_elements
            ));

            return Error::ok();
        }

        Result<rerun::DataCell> Text::to_data_cell(const Text *instances, size_t num_instances) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool *pool = arrow::default_memory_pool();

            auto builder_result = Text::new_arrow_array_builder(pool);
            RR_RETURN_NOT_OK(builder_result.error);
            auto builder = std::move(builder_result.value);
            if (instances && num_instances > 0) {
                RR_RETURN_NOT_OK(
                    Text::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema({arrow::field(Text::NAME, Text::arrow_datatype(), false)});

            rerun::DataCell cell;
            cell.component_name = Text::NAME;
            const auto ipc_result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
            RR_RETURN_NOT_OK(ipc_result.error);
            cell.buffer = std::move(ipc_result.value);

            return cell;
        }
    } // namespace components
} // namespace rerun
