#pragma once

#include <memory>
#include <shared_mutex>
#include <unordered_map>

#include "component_descriptor.hpp"
#include "component_type.hpp"
#include "result.hpp"

namespace arrow {
    class Array;
    class DataType;
} // namespace arrow

namespace rerun {

    /// Thread-safe registry for component types.
    ///
    /// Ensures that each component descriptor is only registered once.
    class ComponentTypeRegistry {
      public:
        ComponentTypeRegistry() = default;

        /// Returns the handle to the registered component type for the given descriptor.
        ///
        /// Registers the component type when first encountered.
        Result<ComponentTypeHandle> get_or_register(
            const ComponentDescriptor& descriptor,
            const std::shared_ptr<arrow::DataType>& arrow_datatype
        );

      private:
        std::shared_mutex mutex_;
        std::unordered_map<ComponentDescriptorHash, ComponentTypeHandle> comp_types_per_descr_;
    };

} // namespace rerun
