#pragma once

#include <array>
#include <vector>

#include "data_cell.hpp"
#include "result.hpp"

namespace rerun {
    /// Generic list of components that are contigious in memory.
    ///
    /// Does *not* own the data, user is responsible for the lifetime independent of how it was
    /// passed in.
    template <typename ComponentType>
    struct ComponentList {
        const ComponentType* data;
        size_t size;

        /// Construct from a raw pointer and size.
        ComponentList(const ComponentType* _data, size_t _size) : data(_data), size(_size) {}

        /// Construct from an std::vector.
        ComponentList(const std::vector<ComponentType>& _data)
            : data(_data.data()), size(_data.size()) {}

        /// Construct from an std::array.
        template <size_t Size>
        ComponentList(const std::array<ComponentType, Size>& _data)
            : data(_data.data()), size(Size) {}

        /// Construct from a C-Array.
        template <size_t Size>
        ComponentList(const ComponentType (&_data)[Size]) : data(_data), size(Size) {}

        /// Creates a Rerun DataCell from this list of components.
        Result<rerun::DataCell> to_data_cell() {
            return ComponentType::to_data_cell(data, size);
        }
    };
} // namespace rerun
