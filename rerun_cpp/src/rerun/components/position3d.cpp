// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/position3d.fbs".

#include "position3d.hpp"

#include "../datatypes/vec3d.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {
    const char Position3D::NAME[] = "rerun.components.Position3D";

    const std::shared_ptr<arrow::DataType>& Position3D::arrow_datatype() {
        static const auto datatype = rerun::datatypes::Vec3D::arrow_datatype();
        return datatype;
    }

    Result<std::shared_ptr<arrow::FixedSizeListBuilder>> Position3D::new_arrow_array_builder(
        arrow::MemoryPool* memory_pool
    ) {
        if (memory_pool == nullptr) {
            return rerun::Error(ErrorCode::UnexpectedNullArgument, "Memory pool is null.");
        }

        return Result(rerun::datatypes::Vec3D::new_arrow_array_builder(memory_pool).value);
    }

    rerun::Error Position3D::fill_arrow_array_builder(
        arrow::FixedSizeListBuilder* builder, const Position3D* elements, size_t num_elements
    ) {
        if (builder == nullptr) {
            return rerun::Error(ErrorCode::UnexpectedNullArgument, "Passed array builder is null.");
        }
        if (elements == nullptr) {
            return rerun::Error(
                ErrorCode::UnexpectedNullArgument,
                "Cannot serialize null pointer to arrow array."
            );
        }

        static_assert(sizeof(rerun::datatypes::Vec3D) == sizeof(Position3D));
        RR_RETURN_NOT_OK(rerun::datatypes::Vec3D::fill_arrow_array_builder(
            builder,
            reinterpret_cast<const rerun::datatypes::Vec3D*>(elements),
            num_elements
        ));

        return Error::ok();
    }

    Result<rerun::DataCell> Position3D::to_data_cell(
        const Position3D* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        auto builder_result = Position3D::new_arrow_array_builder(pool);
        RR_RETURN_NOT_OK(builder_result.error);
        auto builder = std::move(builder_result.value);
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(
                Position3D::fill_arrow_array_builder(builder.get(), instances, num_instances)
            );
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        return rerun::DataCell::create(
            Position3D::NAME,
            Position3D::arrow_datatype(),
            std::move(array)
        );
    }
} // namespace rerun::components
