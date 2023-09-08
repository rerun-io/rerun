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
    class ComponentList {
      public:
        const ComponentType* data;
        size_t size;

      public:
        /// Construct from a single component.
        ///
        /// *Attention*: As with all other constructors, this does *not* take ownership of the data,
        /// you need to ensure that the data outlives the component list.
        ComponentList(const ComponentType& one_and_only) : data(&one_and_only), size(1) {}

        /// Construct from a raw pointer and size.
        ComponentList(const ComponentType* _data, size_t _size) : data(_data), size(_size) {}

        /// Construct from an std::vector.
        ///
        /// *Attention*: As with all other constructors, this does *not* take ownership of the data,
        /// you need to ensure that the data outlives the component list.
        /// In particular, manipulating the passed vector after constructing the component list,
        /// will invalidate it, similar to iterator invalidation.
        ComponentList(const std::vector<ComponentType>& _data)
            : data(_data.data()), size(_data.size()) {}

        /// Construct from an std::array.
        ///
        /// *Attention*: As with all other constructors, this does *not* take ownership of the data,
        /// you need to ensure that the data outlives the component list.
        template <size_t Size>
        ComponentList(const std::array<ComponentType, Size>& _data)
            : data(_data.data()), size(Size) {}

        /// Construct from a C-Array.
        ///
        /// *Attention*: As with all other constructors, this does *not* take ownership of the data,
        /// you need to ensure that the data outlives the component list.
        template <size_t Size>
        ComponentList(const ComponentType (&_data)[Size]) : data(_data), size(Size) {}

        /// Creates a Rerun DataCell from this list of components.
        Result<rerun::DataCell> to_data_cell() const {
            return ComponentType::to_data_cell(data, size);
        }
    };

    /// A type erased version of `ComponentList`.
    class AnonymousComponentList {
      public:
        const void* data;
        size_t size;

      public:
        /// Construct from any parameter that can be converted to a strongly typed component list.
        template <typename ComponentListLikeType>
        AnonymousComponentList(const ComponentListLikeType& component_list_like)
            : AnonymousComponentList(ComponentList(component_list_like)) {}

        /// Construct from a strongly typed component list.
        template <typename ComponentType>
        AnonymousComponentList(const ComponentList<ComponentType>& component_list)
            : data(component_list.data),
              size(component_list.size),
              to_data_cell_func([](const void* _data, size_t _size) {
                  return ComponentType::to_data_cell(
                      reinterpret_cast<const ComponentType*>(_data),
                      _size
                  );
              }) {}

        /// Creates a Rerun DataCell from this list of components.
        Result<rerun::DataCell> to_data_cell() const {
            return to_data_cell_func(data, size);
        }

      private:
        Result<rerun::DataCell> (*to_data_cell_func)(const void*, size_t);
    };
} // namespace rerun
