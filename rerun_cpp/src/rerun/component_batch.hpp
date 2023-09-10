#pragma once

#include <array>
#include <vector>

#include "data_cell.hpp"
#include "result.hpp"

namespace rerun {
    /// Generic list of components that are contiguous in memory.
    ///
    /// Does *not* own the data, user is responsible for the lifetime independent of how it was
    /// passed in.
    template <typename ComponentType>
    class ComponentBatch {
      public:
        const ComponentType* data;
        size_t num_instances;

      public:
        /// Construct from a single component.
        ///
        /// *Attention*: As with all other constructors, this does *not* take ownership of the data,
        /// you need to ensure that the data outlives the component list.
        ComponentBatch(const ComponentType& one_and_only) : data(&one_and_only), num_instances(1) {}

        /// Construct from a raw pointer and size.
        ComponentBatch(const ComponentType* _data, size_t _num_instances)
            : data(_data), num_instances(_num_instances) {}

        /// Construct from an std::vector.
        ///
        /// *Attention*: As with all other constructors, this does *not* take ownership of the data,
        /// you need to ensure that the data outlives the component list.
        /// In particular, manipulating the passed vector after constructing the component list,
        /// will invalidate it, similar to iterator invalidation.
        ComponentBatch(const std::vector<ComponentType>& _data)
            : data(_data.data()), num_instances(_data.size()) {}

        /// Construct from an std::array.
        ///
        /// *Attention*: As with all other constructors, this does *not* take ownership of the data,
        /// you need to ensure that the data outlives the component list.
        template <size_t NumInstances>
        ComponentBatch(const std::array<ComponentType, NumInstances>& _data)
            : data(_data.data()), num_instances(NumInstances) {}

        /// Construct from a C-Array.
        ///
        /// *Attention*: As with all other constructors, this does *not* take ownership of the data,
        /// you need to ensure that the data outlives the component list.
        template <size_t NumInstances>
        ComponentBatch(const ComponentType (&_data)[NumInstances])
            : data(_data), num_instances(NumInstances) {}

        /// Creates a Rerun DataCell from this list of components.
        Result<rerun::DataCell> to_data_cell() const {
            return ComponentType::to_data_cell(data, num_instances);
        }
    };

    /// A type erased version of `ComponentBatch`.
    class AnonymousComponentBatch {
      public:
        const void* data;
        size_t num_instances;

      public:
        /// Construct from any parameter that can be converted to a strongly typed component list.
        template <typename ComponentBatchLikeType>
        AnonymousComponentBatch(const ComponentBatchLikeType& component_list_like)
            : AnonymousComponentBatch(ComponentBatch(component_list_like)) {}

        /// Construct from a strongly typed component list.
        template <typename ComponentType>
        AnonymousComponentBatch(const ComponentBatch<ComponentType>& component_list)
            : data(component_list.data),
              num_instances(component_list.num_instances),
              to_data_cell_func([](const void* _data, size_t _num_instances) {
                  return ComponentType::to_data_cell(
                      reinterpret_cast<const ComponentType*>(_data),
                      _num_instances
                  );
              }) {}

        /// Creates a Rerun DataCell from this list of components.
        Result<rerun::DataCell> to_data_cell() const {
            return to_data_cell_func(data, num_instances);
        }

      private:
        Result<rerun::DataCell> (*to_data_cell_func)(const void*, size_t);
    };
} // namespace rerun
