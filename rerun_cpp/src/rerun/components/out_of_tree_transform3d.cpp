// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/out_of_tree_transform3d.fbs".

#include "out_of_tree_transform3d.hpp"

#include "../datatypes/transform3d.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {
    const char OutOfTreeTransform3D::NAME[] = "rerun.components.OutOfTreeTransform3D";

    const std::shared_ptr<arrow::DataType>& OutOfTreeTransform3D::arrow_datatype() {
        static const auto datatype = rerun::datatypes::Transform3D::arrow_datatype();
        return datatype;
    }

    rerun::Error OutOfTreeTransform3D::fill_arrow_array_builder(
        arrow::DenseUnionBuilder* builder, const OutOfTreeTransform3D* elements, size_t num_elements
    ) {
        static_assert(sizeof(rerun::datatypes::Transform3D) == sizeof(OutOfTreeTransform3D));
        RR_RETURN_NOT_OK(rerun::datatypes::Transform3D::fill_arrow_array_builder(
            builder,
            reinterpret_cast<const rerun::datatypes::Transform3D*>(elements),
            num_elements
        ));

        return Error::ok();
    }

    Result<rerun::DataCell> OutOfTreeTransform3D::to_data_cell(
        const OutOfTreeTransform3D* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(arrow_datatype(), pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(OutOfTreeTransform3D::fill_arrow_array_builder(
                static_cast<arrow::DenseUnionBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        return rerun::DataCell::create(
            OutOfTreeTransform3D::NAME,
            OutOfTreeTransform3D::arrow_datatype(),
            std::move(array)
        );
    }
} // namespace rerun::components
