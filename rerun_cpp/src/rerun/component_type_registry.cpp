#include "component_type_registry.hpp"

#include <mutex>

namespace rerun {

    /// Returns the handle to the registered component type for the given descriptor.
    ///
    /// Registers the component type when first encountered.
    Result<ComponentTypeHandle> ComponentTypeRegistry::get_or_register(
        const ComponentDescriptor& descriptor,
        const std::shared_ptr<arrow::DataType>& arrow_datatype
    ) {
        {
            // The read-only operation in this scope can be done concurrently.
            std::shared_lock lock(mutex_);

            const auto descr_hash = descriptor.hashed();
            if (const auto search = comp_types_per_descr_.find(descr_hash);
                search != comp_types_per_descr_.end()) {
                return search->second;
            }
        }

        // Only one thread is allowed to do the initial registration of a new component type.
        std::unique_lock lock(mutex_);

        const auto descr_hash = descriptor.hashed();
        if (const auto search = comp_types_per_descr_.find(descr_hash);
            search != comp_types_per_descr_.end()) {
            return search->second;
        }

        const Result<ComponentTypeHandle> comp_type_handle_result =
            ComponentType(descriptor, arrow_datatype).register_component();
        RR_RETURN_NOT_OK(comp_type_handle_result.error);

        comp_types_per_descr_.insert({descr_hash, comp_type_handle_result.value});
        return comp_type_handle_result.value;
    }

} // namespace rerun
