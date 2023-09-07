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
            ///
            /// Typically only a single indicator component is ever used. `num_instances` is merely
            /// there to match the commonly used interface.
            /// Since indicator components have no data, strongly typed null can be passed in
            /// instead of a valid pointer.
            static Result<rerun::DataCell> to_data_cell(
                const IndicatorComponent<Name>*, size_t num_instances
            ) {
                return rerun::create_indicator_component(Name, num_instances);
            }
        };
    } // namespace components
} // namespace rerun
