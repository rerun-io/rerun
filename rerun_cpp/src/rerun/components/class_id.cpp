// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/class_id.fbs".

#include "class_id.hpp"

#include "../datatypes/class_id.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {
    const char ClassId::NAME[] = "rerun.components.ClassId";

    const std::shared_ptr<arrow::DataType>& ClassId::arrow_datatype() {
        static const auto datatype = rerun::datatypes::ClassId::arrow_datatype();
        return datatype;
    }

    rerun::Error ClassId::fill_arrow_array_builder(
        arrow::UInt16Builder* builder, const ClassId* elements, size_t num_elements
    ) {
        static_assert(sizeof(rerun::datatypes::ClassId) == sizeof(ClassId));
        RR_RETURN_NOT_OK(rerun::datatypes::ClassId::fill_arrow_array_builder(
            builder,
            reinterpret_cast<const rerun::datatypes::ClassId*>(elements),
            num_elements
        ));

        return Error::ok();
    }

    Result<rerun::DataCell> ClassId::to_data_cell(const ClassId* instances, size_t num_instances) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(arrow_datatype(), pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(ClassId::fill_arrow_array_builder(
                static_cast<arrow::UInt16Builder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        return rerun::DataCell::create(ClassId::NAME, ClassId::arrow_datatype(), std::move(array));
    }
} // namespace rerun::components
