#include "component_batch.hpp"

#include <arrow/array/array_base.h>
#include <arrow/c/bridge.h>

#include "c/rerun.h"

namespace rerun {
    /// Creates a new component batch from a collection of component instances.
    ///
    /// Automatically registers the component type the first time this type is encountered.
    Result<ComponentBatch> ComponentBatch::from_arrow_array(
        std::shared_ptr<arrow::Array> array, const ComponentDescriptor& descriptor
    ) {
        static std::unordered_map<ComponentDescriptorHash, ComponentTypeHandle>
            comp_types_per_descr;

        ComponentTypeHandle comp_type_handle;

        auto descr_hash = descriptor.hashed();

        auto search = comp_types_per_descr.find(descr_hash);
        if (search != comp_types_per_descr.end()) {
            comp_type_handle = search->second;
        } else {
            auto comp_type = ComponentType(descriptor, array->type());

            const Result<ComponentTypeHandle> comp_type_handle_result =
                comp_type.register_component();
            RR_RETURN_NOT_OK(comp_type_handle_result.error);

            comp_type_handle = comp_type_handle_result.value;
            comp_types_per_descr.insert({descr_hash, comp_type_handle});
        }

        ComponentBatch component_batch;
        component_batch.array = std::move(array);
        component_batch.component_type = comp_type_handle;
        return component_batch;
    }

    Error ComponentBatch::to_c_ffi_struct(rr_component_batch& out_component_batch) const {
        if (array == nullptr) {
            return Error(ErrorCode::UnexpectedNullArgument, "array is null");
        }

        out_component_batch.component_type = component_type;
        return arrow::ExportArray(*array, &out_component_batch.array, nullptr);
    }

    size_t ComponentBatch::length() const {
        return static_cast<size_t>(array->length());
    }
} // namespace rerun
