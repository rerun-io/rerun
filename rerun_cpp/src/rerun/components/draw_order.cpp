// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/draw_order.fbs"

#include "draw_order.hpp"

#include "../arrow.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace components {
        const char* DrawOrder::NAME = "rerun.draw_order";

        const std::shared_ptr<arrow::DataType>& DrawOrder::to_arrow_datatype() {
            static const auto datatype = arrow::float32();
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::FloatBuilder>> DrawOrder::new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::FloatBuilder>(memory_pool));
        }

        arrow::Status DrawOrder::fill_arrow_array_builder(
            arrow::FloatBuilder* builder, const DrawOrder* elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            static_assert(sizeof(*elements) == sizeof(elements->value));
            ARROW_RETURN_NOT_OK(
                builder->AppendValues(&elements->value, static_cast<int64_t>(num_elements))
            );

            return arrow::Status::OK();
        }

        arrow::Result<rerun::DataCell> DrawOrder::to_data_cell(
            const DrawOrder* instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool* pool = arrow::default_memory_pool();

            ARROW_ASSIGN_OR_RAISE(auto builder, DrawOrder::new_arrow_array_builder(pool));
            if (instances && num_instances > 0) {
                ARROW_RETURN_NOT_OK(
                    DrawOrder::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema =
                arrow::schema({arrow::field(DrawOrder::NAME, DrawOrder::to_arrow_datatype(), false)}
                );

            rerun::DataCell cell;
            cell.component_name = DrawOrder::NAME;
            ARROW_ASSIGN_OR_RAISE(
                cell.buffer,
                rerun::ipc_from_table(*arrow::Table::Make(schema, {array}))
            );

            return cell;
        }
    } // namespace components
} // namespace rerun
