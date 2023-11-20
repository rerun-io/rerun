// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/components/mesh_properties.fbs".

#include "mesh_properties.hpp"

#include "../datatypes/mesh_properties.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::components {
    const char MeshProperties::NAME[] = "rerun.components.MeshProperties";

    const std::shared_ptr<arrow::DataType>& MeshProperties::arrow_datatype() {
        static const auto datatype = rerun::datatypes::MeshProperties::arrow_datatype();
        return datatype;
    }

    rerun::Error MeshProperties::fill_arrow_array_builder(
        arrow::StructBuilder* builder, const MeshProperties* elements, size_t num_elements
    ) {
        static_assert(sizeof(rerun::datatypes::MeshProperties) == sizeof(MeshProperties));
        RR_RETURN_NOT_OK(rerun::datatypes::MeshProperties::fill_arrow_array_builder(
            builder,
            reinterpret_cast<const rerun::datatypes::MeshProperties*>(elements),
            num_elements
        ));

        return Error::ok();
    }

    Result<rerun::DataCell> MeshProperties::to_data_cell(
        const MeshProperties* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(arrow_datatype(), pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(MeshProperties::fill_arrow_array_builder(
                static_cast<arrow::StructBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        DataCell cell;
        cell.num_instances = num_instances;
        cell.component_name = MeshProperties::NAME;
        cell.datatype = MeshProperties::arrow_datatype().get();
        cell.array = std::move(array);
        return cell;
    }
} // namespace rerun::components
