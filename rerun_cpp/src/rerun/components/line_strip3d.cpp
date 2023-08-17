// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/line_strip3d.fbs"

#include "line_strip3d.hpp"

#include "../arrow.hpp"
#include "../datatypes/vec3d.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace components {
        const char *LineStrip3D::NAME = "rerun.linestrip3d";

        const std::shared_ptr<arrow::DataType> &LineStrip3D::to_arrow_datatype() {
            static const auto datatype = arrow::list(
                arrow::field("item", rerun::datatypes::Vec3D::to_arrow_datatype(), false)
            );
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::ListBuilder>> LineStrip3D::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
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
            arrow::ListBuilder *builder, const LineStrip3D *elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            auto value_builder =
                static_cast<arrow::FixedSizeListBuilder *>(builder->value_builder());
            ARROW_RETURN_NOT_OK(builder->Reserve(static_cast<int64_t>(num_elements)));
            ARROW_RETURN_NOT_OK(value_builder->Reserve(static_cast<int64_t>(num_elements * 2)));

            for (size_t elem_idx = 0; elem_idx < num_elements; elem_idx += 1) {
                const auto &element = elements[elem_idx];
                ARROW_RETURN_NOT_OK(builder->Append());
                if (element.points.data()) {
                    ARROW_RETURN_NOT_OK(rerun::datatypes::Vec3D::fill_arrow_array_builder(
                        value_builder,
                        element.points.data(),
                        element.points.size()
                    ));
                }
            }

            return arrow::Status::OK();
        }

        Result<rerun::DataCell> LineStrip3D::to_data_cell(
            const LineStrip3D *instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool *pool = arrow::default_memory_pool();

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
            const auto result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
            if (result.is_err()) {
                return result.error;
            }
            cell.buffer = std::move(result.value);

            return cell;
        }
    } // namespace components
} // namespace rerun
