#pragma once

#include "arrow.hpp"
#include "data_cell.hpp"

namespace rerun {
    namespace components {
        /// Indicator component used by archetypes when converting them to component lists.
        ///
        /// This is done in order to track how a collection of component was logged.
        template <const char Name[]>
        struct IndicatorComponent {
          public:
            IndicatorComponent() = default;

            /// Creates a Rerun DataCell from an array of IndicatorComponent components.
            static Result<rerun::DataCell> to_data_cell(const IndicatorComponent<Name>*, size_t) {
                return rerun::DataCell::create_indicator_component(Name);
            }
        };
    } // namespace components
} // namespace rerun
