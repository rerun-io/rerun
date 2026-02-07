#include "component_batch.hpp"
#include "component_column.hpp"
#include "component_type_registry.hpp"

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
        static ComponentTypeRegistry comp_type_registry;

        const Result<ComponentTypeHandle> comp_type_handle =
            comp_type_registry.get_or_register(descriptor, array->type());
        RR_RETURN_NOT_OK(comp_type_handle.error);

        ComponentBatch component_batch;
        component_batch.array = std::move(array);
        component_batch.component_type = comp_type_handle.value;
        return component_batch;
    }

    Result<ComponentColumn> ComponentBatch::partitioned(const Collection<uint32_t>& lengths) && {
        // Can't define this method in the header because it needs to know about `ComponentColumn`.
        return ComponentColumn::from_batch_with_lengths(std::move(*this), lengths);
    }

    Result<ComponentColumn> ComponentBatch::partitioned() && {
        // Can't define this method in the header because it needs to know about `ComponentColumn`.
        return std::move(*this).partitioned(std::vector<uint32_t>(length(), 1));
    }

    Result<ComponentColumn> ComponentBatch::partitioned(const Collection<uint32_t>& lengths
    ) const& {
        // Can't define this method in the header because it needs to know about `ComponentColumn`.
        return ComponentColumn::from_batch_with_lengths(*this, lengths);
    }

    Result<ComponentColumn> ComponentBatch::partitioned() const& {
        // Can't define this method in the header because it needs to know about `ComponentColumn`.
        return partitioned(std::vector<uint32_t>(length(), 1));
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
