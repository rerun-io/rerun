// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/radius.fbs"

#include "radius.hpp"

#include "../rerun.hpp"

#include <arrow/api.h>

namespace rr {
    namespace components {
        const char* Radius::NAME = "rerun.radius";

        const std::shared_ptr<arrow::DataType>& Radius::to_arrow_datatype() {
            static const auto datatype = arrow::float32();
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::FloatBuilder>> Radius::new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::FloatBuilder>(memory_pool));
        }

        arrow::Status Radius::fill_arrow_array_builder(
            arrow::FloatBuilder* builder, const Radius* elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            static_assert(sizeof(*elements) == sizeof(elements->value));
            ARROW_RETURN_NOT_OK(builder->AppendValues(&elements->value, num_elements));

            return arrow::Status::OK();
        }

        arrow::Result<rr::DataCell> Radius::to_data_cell(
            const Radius* components, size_t num_components
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool* pool = arrow::default_memory_pool();

            ARROW_ASSIGN_OR_RAISE(auto builder, Radius::new_arrow_array_builder(pool));
            if (components && num_components > 0) {
                ARROW_RETURN_NOT_OK(
                    Radius::fill_arrow_array_builder(builder.get(), components, num_components)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema =
                arrow::schema({arrow::field(Radius::NAME, Radius::to_arrow_datatype(), false)});

            rr::DataCell cell;
            cell.component_name = Radius::NAME;
            ARROW_ASSIGN_OR_RAISE(
                cell.buffer,
                rr::ipc_from_table(*arrow::Table::Make(schema, {array}))
            );

            return cell;
        }
    } // namespace components
} // namespace rr
