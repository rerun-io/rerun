// NOTE: This file was autogenerated by re_types_builder; DO NOT EDIT.
// Based on "crates/re_types/definitions/rerun/components/point3d.fbs"

#include "point3d.hpp"

#include "../arrow.hpp"
#include "../datatypes/vec3d.hpp"

#include <arrow/api.h>

namespace rerun {
    namespace components {
        const char *Point3D::NAME = "rerun.point3d";

        const std::shared_ptr<arrow::DataType> &Point3D::to_arrow_datatype() {
            static const auto datatype = rerun::datatypes::Vec3D::to_arrow_datatype();
            return datatype;
        }

        arrow::Result<std::shared_ptr<arrow::FixedSizeListBuilder>>
            Point3D::new_arrow_array_builder(arrow::MemoryPool *memory_pool) {
            if (!memory_pool) {
                return arrow::Status::Invalid("Memory pool is null.");
            }

            return arrow::Result(
                rerun::datatypes::Vec3D::new_arrow_array_builder(memory_pool).ValueOrDie()
            );
        }

        arrow::Status Point3D::fill_arrow_array_builder(
            arrow::FixedSizeListBuilder *builder, const Point3D *elements, size_t num_elements
        ) {
            if (!builder) {
                return arrow::Status::Invalid("Passed array builder is null.");
            }
            if (!elements) {
                return arrow::Status::Invalid("Cannot serialize null pointer to arrow array.");
            }

            static_assert(sizeof(rerun::datatypes::Vec3D) == sizeof(Point3D));
            ARROW_RETURN_NOT_OK(rerun::datatypes::Vec3D::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rerun::datatypes::Vec3D *>(elements),
                num_elements
            ));

            return arrow::Status::OK();
        }

        Result<rerun::DataCell> Point3D::to_data_cell(
            const Point3D *instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool *pool = arrow::default_memory_pool();

            ARROW_ASSIGN_OR_RAISE(auto builder, Point3D::new_arrow_array_builder(pool));
            if (instances && num_instances > 0) {
                ARROW_RETURN_NOT_OK(
                    Point3D::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema =
                arrow::schema({arrow::field(Point3D::NAME, Point3D::to_arrow_datatype(), false)});

            rerun::DataCell cell;
            cell.component_name = Point3D::NAME;
            const auto result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
            if (result.is_err()) {
                return result.error;
            }
            cell.buffer = std::move(result.value);

            return cell;
        }
    } // namespace components
} // namespace rerun
