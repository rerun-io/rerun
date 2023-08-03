// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/testing/components/fuzzy_deps.fbs"

#include "string_component.hpp"

#include "../arrow.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace components {
        const char* StringComponent::NAME = "rerun.testing.components.StringComponent";

        const std::shared_ptr<arrow::DataType>& StringComponent::to_arrow_datatype() {
            static const auto datatype = arrow::utf8();
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::StringBuilder>>
            StringComponent::new_arrow_array_builder(arrow::MemoryPool* memory_pool) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::StringBuilder>(memory_pool));
        }

        arrow::Status StringComponent::fill_arrow_array_builder(
            arrow::StringBuilder* builder, const StringComponent* elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            ARROW_RETURN_NOT_OK(builder->Reserve(num_elements));
            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                ARROW_RETURN_NOT_OK(builder->Append(elements[elem_idx].value));
            }

            return arrow::Status::OK();
        }

        arrow::Result<rerun::DataCell> StringComponent::to_data_cell(
            const StringComponent* instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool* pool = arrow::default_memory_pool();

            ARROW_ASSIGN_OR_RAISE(auto builder, StringComponent::new_arrow_array_builder(pool));
            if (instances && num_instances > 0) {
                ARROW_RETURN_NOT_OK(StringComponent::fill_arrow_array_builder(
                    builder.get(),
                    instances,
                    num_instances
                ));
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema(
                {arrow::field(StringComponent::NAME, StringComponent::to_arrow_datatype(), false)}
            );

            rerun::DataCell cell;
            cell.component_name = StringComponent::NAME;
            ARROW_ASSIGN_OR_RAISE(
                cell.buffer,
                rerun::ipc_from_table(*arrow::Table::Make(schema, {array}))
            );

            return cell;
        }
    } // namespace components
} // namespace rerun
