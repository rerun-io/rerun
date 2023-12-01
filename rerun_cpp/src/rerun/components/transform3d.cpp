// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/transform3d.fbs".

#include "transform3d.hpp"

#include "../datatypes/transform3d.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {}

namespace rerun {
    const std::shared_ptr<arrow::DataType>& Loggable<components::Transform3D>::arrow_datatype() {
        static const auto datatype = Loggable<rerun::datatypes::Transform3D>::arrow_datatype();
        return datatype;
    }

    rerun::Error Loggable<components::Transform3D>::fill_arrow_array_builder(
        arrow::DenseUnionBuilder* builder, const components::Transform3D* elements,
        size_t num_elements
    ) {
        static_assert(sizeof(rerun::datatypes::Transform3D) == sizeof(components::Transform3D));
        RR_RETURN_NOT_OK(Loggable<rerun::datatypes::Transform3D>::fill_arrow_array_builder(
            builder,
            reinterpret_cast<const rerun::datatypes::Transform3D*>(elements),
            num_elements
        ));

        return Error::ok();
    }

    Result<std::shared_ptr<arrow::Array>> Loggable<components::Transform3D>::to_arrow(
        const components::Transform3D* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(Loggable<components::Transform3D>::fill_arrow_array_builder(
                static_cast<arrow::DenseUnionBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));
        return array;
    }
} // namespace rerun
