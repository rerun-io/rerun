// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/keypoint_id.fbs"

#include "keypoint_id.hpp"

#include "../rerun.hpp"

#include <arrow/api.h>

namespace rr {
    namespace components {
        const char* KeypointId::NAME = "rerun.keypoint_id";

        const std::shared_ptr<arrow::DataType>& KeypointId::to_arrow_datatype() {
            static const auto datatype = arrow::uint16();
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::UInt16Builder>> KeypointId::new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::UInt16Builder>(memory_pool));
        }

        arrow::Status KeypointId::fill_arrow_array_builder(
            arrow::UInt16Builder* builder, const KeypointId* elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            static_assert(sizeof(*elements) == sizeof(elements->id));
            ARROW_RETURN_NOT_OK(builder->AppendValues(&elements->id, num_elements));

            return arrow::Status::OK();
        }

        arrow::Result<rr::DataCell> KeypointId::to_data_cell(
            const KeypointId* components, size_t num_components
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool* pool = arrow::default_memory_pool();

            ARROW_ASSIGN_OR_RAISE(auto builder, KeypointId::new_arrow_array_builder(pool));
            if (components && num_components > 0) {
                ARROW_RETURN_NOT_OK(
                    KeypointId::fill_arrow_array_builder(builder.get(), components, num_components)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema(
                {arrow::field(KeypointId::NAME, KeypointId::to_arrow_datatype(), false)}
            );

            rr::DataCell cell;
            cell.component_name = KeypointId::NAME;
            ARROW_ASSIGN_OR_RAISE(
                cell.buffer,
                rr::ipc_from_table(*arrow::Table::Make(schema, {array}))
            );

            return cell;
        }
    } // namespace components
} // namespace rr
