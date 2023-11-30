#pragma once

#include <memory> // std::shared_ptr
#include "loggable.hpp"
#include "result.hpp"

namespace arrow {
    class DataType;
    class Array;
} // namespace arrow

namespace rerun::components {
    /// Arrow data type shared by all instances of IndicatorComponent.
    const std::shared_ptr<arrow::DataType>& indicator_arrow_datatype();

    /// Returns an arrow array for a single indicator component.
    const std::shared_ptr<arrow::Array>& indicator_arrow_array();

    /// Indicator component used by archetypes when converting them to component lists.
    ///
    /// This is done in order to track how a collection of component was logged.
    template <const char Name[]>
    struct IndicatorComponent {};
} // namespace rerun::components

namespace rerun {
    /// \private
    template <const char Name_[]>
    struct Loggable<components::IndicatorComponent<Name_>> {
        /// Returns the name of this type.
        static constexpr const char* Name = Name_;

        /// Returns the arrow data type this type corresponds to.
        static const std::shared_ptr<arrow::DataType>& arrow_datatype() {
            return components::indicator_arrow_datatype();
        }

        /// Creates an arrow DataCell from an array of IndicatorComponent components.
        static Result<std::shared_ptr<arrow::Array>> to_arrow(
            const components::IndicatorComponent<Name_>*, size_t
        ) {
            return components::indicator_arrow_array();
        }
    };
} // namespace rerun
