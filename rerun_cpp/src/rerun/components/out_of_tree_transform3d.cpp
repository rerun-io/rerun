// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/out_of_tree_transform3d.fbs".

#include "out_of_tree_transform3d.hpp"

#include "../arrow.hpp"
#include "../datatypes/transform3d.hpp"

#include <arrow/builder.h>
#include <arrow/table.h>
#include <arrow/type_fwd.h>

namespace rerun {
    namespace components {
        const char OutOfTreeTransform3D::NAME[] = "rerun.components.OutOfTreeTransform3D";

        const std::shared_ptr<arrow::DataType> &OutOfTreeTransform3D::arrow_datatype() {
            static const auto datatype = rerun::datatypes::Transform3D::arrow_datatype();
            return datatype;
        }

        Result<std::shared_ptr<arrow::DenseUnionBuilder>>
            OutOfTreeTransform3D::new_arrow_array_builder(arrow::MemoryPool *memory_pool) {
            if (memory_pool == nullptr) {
                return Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
            }

            return Result(rerun::datatypes::Transform3D::new_arrow_array_builder(memory_pool).value
            );
        }

        Error OutOfTreeTransform3D::fill_arrow_array_builder(
            arrow::DenseUnionBuilder *builder, const OutOfTreeTransform3D *elements,
            size_t num_elements
        ) {
            if (builder == nullptr) {
                return Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.");
            }
            if (elements == nullptr) {
                return Error(
                    ErrorCode::UnexpectedNullArgument,
                    "Cannot serialize null pointer to arrow array."
                );
            }

            static_assert(sizeof(rerun::datatypes::Transform3D) == sizeof(OutOfTreeTransform3D));
            RR_RETURN_NOT_OK(rerun::datatypes::Transform3D::fill_arrow_array_builder(
                builder,
                reinterpret_cast<const rerun::datatypes::Transform3D *>(elements),
                num_elements
            ));

            return Error::ok();
        }

        Result<rerun::DataCell> OutOfTreeTransform3D::to_data_cell(
            const OutOfTreeTransform3D *instances, size_t num_instances
        ) {
            // TODO(andreas): Allow configuring the memory pool.
            arrow::MemoryPool *pool = arrow::default_memory_pool();

            auto builder_result = OutOfTreeTransform3D::new_arrow_array_builder(pool);
            RR_RETURN_NOT_OK(builder_result.error);
            auto builder = std::move(builder_result.value);
            if (instances && num_instances > 0) {
                RR_RETURN_NOT_OK(OutOfTreeTransform3D::fill_arrow_array_builder(
                    builder.get(),
                    instances,
                    num_instances
                ));
            }
            std::shared_ptr<arrow::Array> array;
            ARROW_RETURN_NOT_OK(builder->Finish(&array));

            auto schema = arrow::schema({arrow::field(
                OutOfTreeTransform3D::NAME,
                OutOfTreeTransform3D::arrow_datatype(),
                false
            )});

            rerun::DataCell cell;
            cell.component_name = OutOfTreeTransform3D::NAME;
            const auto ipc_result = rerun::ipc_from_table(*arrow::Table::Make(schema, {array}));
            RR_RETURN_NOT_OK(ipc_result.error);
            cell.buffer = std::move(ipc_result.value);

            return cell;
        }
    } // namespace components
} // namespace rerun
