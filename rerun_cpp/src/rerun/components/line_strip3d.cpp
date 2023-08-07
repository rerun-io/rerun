// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/line_strip3d.fbs"

#include "line_strip3d.hpp"

#include "../arrow.hpp"
#include "../datatypes/vec3d.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace components {
        const char* LineStrip3D::NAME = "rerun.linestrip3d";

        const std::shared_ptr<arrow::DataType>& LineStrip3D::to_arrow_datatype() {
            static const auto datatype = arrow::list(
                arrow::field("item", rerun::datatypes::Vec3D::to_arrow_datatype(), false, nullptr)
            );
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::ListBuilder>> LineStrip3D::new_arrow_array_builder(
            arrow::MemoryPool* memory_pool
        ) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(std::make_shared<arrow::ListBuilder>(
                memory_pool,
                rerun::datatypes::Vec3D::new_arrow_array_builder(memory_pool).ValueOrDie()
            ));
        }

        arrow::Status LineStrip3D::fill_arrow_array_builder(
            arrow::ListBuilder* builder, const LineStrip3D* elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            return arrow::Status::NotImplemented(
                "TODO(andreas): custom data types in lists/fixedsizelist are not yet implemented"
            );

            return arrow::Status::OK();
        }

        arrow::Result<rerun::DataCell> LineStrip3D::to_data_cell(
            const LineStrip3D* instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool* pool = arrow::default_memory_pool();

            ARROW_ASSIGN_OR_RAISE(auto builder, LineStrip3D::new_arrow_array_builder(pool));
            if (instances && num_instances > 0) {
                ARROW_RETURN_NOT_OK(
                    LineStrip3D::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema(
                {arrow::field(LineStrip3D::NAME, LineStrip3D::to_arrow_datatype(), false)}
            );

            rerun::DataCell cell;
            cell.component_name = LineStrip3D::NAME;
            ARROW_ASSIGN_OR_RAISE(
                cell.buffer,
                rerun::ipc_from_table(*arrow::Table::Make(schema, {array}))
            );

            return cell;
        }
    } // namespace components
} // namespace rerun
