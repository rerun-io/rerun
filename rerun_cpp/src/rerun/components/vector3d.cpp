// DO NOT EDIT!: This file was autogenerated by re_types_builder in
// crates/re_types_builder/src/codegen/cpp/mod.rs:54 Based on
// "crates/re_types/definitions/rerun/components/vector3d.fbs"

#include "vector3d.hpp"

#include "../arrow.hpp"
#include "../datatypes/vec3d.hpp"

#include <arrow/builder.h>
#include <arrow/table.h>
#include <arrow/type_fwd.h>

namespace rerun {
    namespace components {
        const char *Vector3D::NAME = "rerun.components.Vector3D";

        const std::shared_ptr<arrow::DataType> &Vector3D::arrow_datatype() {
            static const auto datatype = rerun::datatypes::Vec3D::arrow_datatype();
            return datatype;
        }

        Result<std::shared_ptr<arrow::FixedSizeListBuilder>> Vector3D::new_arrow_array_builder(
            arrow::MemoryPool *memory_pool
        ) {
            if (!memory_pool) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(rerun::datatypes::Vec3D::new_arrow_array_builder(memory_pool).value);
        }

        Error Vector3D::fill_arrow_array_builder(
            arrow::FixedSizeListBuilder *builder, const Vector3D *elements, size_t num_elements
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

            static_assert(sizeof(rerun::datatypes::Vec3D) == sizeof(Vector3D));
            RR_RETURN_NOT_OK(rerun::datatypes::Vec3D::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rerun::datatypes::Vec3D *>(elements),
                num_elements
            ));

            return Error::ok();
        }

        Result<rerun::DataCell> Vector3D::to_data_cell(
            const Vector3D *instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool *pool = arrow::default_memory_pool();

            auto builder_result = Vector3D::new_arrow_array_builder(pool);
            RR_RETURN_NOT_OK(builder_result.error);
            auto builder = std::move(builder_result.value);
            if (instances && num_instances > 0) {
                RR_RETURN_NOT_OK(
                    Vector3D::fill_arrow_array_builder(builder.get(), instances, num_instances)
                );
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema =
                arrow::schema({arrow::field(Vector3D::NAME, Vector3D::arrow_datatype(), false)});

            rerun::DataCell cell;
            cell.component_name = Vector3D::NAME;
            const auto ipc_result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
            RR_RETURN_NOT_OK(ipc_result.error);
            cell.buffer = std::move(ipc_result.value);

            return cell;
        }
    } // namespace components
} // namespace rerun
