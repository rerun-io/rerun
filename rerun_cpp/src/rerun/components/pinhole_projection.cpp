// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/pinhole_projection.fbs".

#include "pinhole_projection.hpp"

#include "../datatypes/mat3x3.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {
    const char PinholeProjection::NAME[] = "rerun.components.PinholeProjection";

    const std::shared_ptr<arrow::DataType>& PinholeProjection::arrow_datatype() {
        static const auto datatype = rerun::datatypes::Mat3x3::arrow_datatype();
        return datatype;
    }

    rerun::Error PinholeProjection::fill_arrow_array_builder(
        arrow::FixedSizeListBuilder* builder, const PinholeProjection* elements, size_t num_elements
    ) {
        static_assert(sizeof(rerun::datatypes::Mat3x3) == sizeof(PinholeProjection));
        RR_RETURN_NOT_OK(rerun::datatypes::Mat3x3::fill_arrow_array_builder(
            builder,
            reinterpret_cast<const rerun::datatypes::Mat3x3*>(elements),
            num_elements
        ));

        return Error::ok();
    }

    Result<rerun::DataCell> PinholeProjection::to_data_cell(
        const PinholeProjection* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(arrow_datatype(), pool));
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(PinholeProjection::fill_arrow_array_builder(
                static_cast<arrow::FixedSizeListBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        return rerun::DataCell::create(
            PinholeProjection::NAME,
            PinholeProjection::arrow_datatype(),
            std::move(array)
        );
    }
} // namespace rerun::components
