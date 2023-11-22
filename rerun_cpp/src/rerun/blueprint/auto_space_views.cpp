// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/auto_space_views.fbs".

#include "auto_space_views.hpp"

#include <arrow/builder.h>
#include <arrow/type_fwd.h>

namespace rerun::blueprint {}

namespace rerun {
    const std::shared_ptr<arrow::DataType>& Loggable<blueprint::AutoSpaceViews>::arrow_datatype() {
        static const auto datatype = arrow::boolean();
        return datatype;
    }

    rerun::Error Loggable<blueprint::AutoSpaceViews>::fill_arrow_array_builder(
        arrow::BooleanBuilder* builder, const blueprint::AutoSpaceViews* elements,
        size_t num_elements
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

        static_assert(sizeof(*elements) == sizeof(elements->enabled));
        ARROW_RETURN_NOT_OK(builder->AppendValues(
            reinterpret_cast<const uint8_t*>(&elements->enabled),
            static_cast<int64_t>(num_elements)
        ));

        return Error::ok();
    }

    Result<rerun::DataCell> Loggable<blueprint::AutoSpaceViews>::to_data_cell(
        const blueprint::AutoSpaceViews* instances, size_t num_instances
    ) {
        // TODO(andreas): Allow configuring the memory pool.
        arrow::MemoryPool* pool = arrow::default_memory_pool();
        auto datatype = arrow_datatype();

        ARROW_ASSIGN_OR_RAISE(auto builder, arrow::MakeBuilder(datatype, pool))
        if (instances && num_instances > 0) {
            RR_RETURN_NOT_OK(Loggable<blueprint::AutoSpaceViews>::fill_arrow_array_builder(
                static_cast<arrow::BooleanBuilder*>(builder.get()),
                instances,
                num_instances
            ));
        }
        std::shared_ptr<arrow::Array> array;
        ARROW_RETURN_NOT_OK(builder->Finish(&array));

        static const Result<ComponentTypeHandle> component_type =
            ComponentType(Name, datatype).register_component();
        RR_RETURN_NOT_OK(component_type.error);

        DataCell cell;
        cell.num_instances = num_instances;
        cell.array = std::move(array);
        cell.component_type = component_type.value;
        return cell;
    }
} // namespace rerun
