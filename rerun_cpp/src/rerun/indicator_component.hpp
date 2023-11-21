#pragma once

#include <memory>
#include "data_cell.hpp"

namespace arrow {
    class DataType;
};

namespace rerun::components {
    /// Arrow data type shared by all instances of IndicatorComponent.
    const std::shared_ptr<arrow::DataType>& indicator_arrow_datatype();

    /// Returns an arrow array for a single indicator component.
    const std::shared_ptr<arrow::Array>& indicator_arrow_array();

    /// Indicator component used by archetypes when converting them to component lists.
    ///
    /// This is done in order to track how a collection of component was logged.
    template <const char Name[]>
    struct IndicatorComponent {
        /// Creates a Rerun DataCell from an array of IndicatorComponent components.
        static Result<rerun::DataCell> to_data_cell(const IndicatorComponent<Name>*, size_t) {
            // Lazily register the component type (only once).
            static const Result<ComponentTypeHandle> component_type =
                ComponentType(Name, indicator_arrow_datatype()).register_component();
            RR_RETURN_NOT_OK(component_type.error);

            rerun::DataCell cell;
            cell.num_instances = 1;
            cell.array = indicator_arrow_array();
            cell.component_type = component_type.value;
            return cell;
        }
    };
} // namespace rerun::components
