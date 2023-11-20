// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/material.fbs".

#include "material.hpp"

#include "../datatypes/material.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {
    const char Material::NAME[] = "rerun.components.Material";

    const std::shared_ptr<arrow::DataType>& Material::arrow_datatype() {
        static const auto datatype = rerun::datatypes::Material::arrow_datatype();
        return datatype;
    }

    rerun::Error Material::fill_arrow_array_builder(
        arrow::StructBuilder* builder, const Material* elements, size_t num_elements
    ) {
        static_assert(sizeof(rerun::datatypes::Material) == sizeof(Material));
        RR_RETURN_NOT_OK(rerun::datatypes::Material::fill_arrow_array_builder(
            builder,
            reinterpret_cast<const rerun::datatypes::Material*>(elements),
            num_elements
        ));

        return Error::ok();
    }

    Result<rerun::DataCell> Material::to_data_cell(
        const Material* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(arrow_datatype(), pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(Material::fill_arrow_array_builder(
                static_cast<arrow::StructBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        DataCell cell;
        cell.num_instances = num_instances;
        cell.component_name = Material::NAME;
        cell.array = std::move(array);
        return cell;
    }
} // namespace rerun::components
