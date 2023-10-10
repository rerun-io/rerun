// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/bar_chart.fbs".

#pragma once

#include "../arrow.hpp"
#include "../component_batch.hpp"
#include "../components/tensor_data.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun {
    namespace archetypes {
        /// **Archetype**: A bar chart.
        ///
        /// The x values will be the indices of the array, and the bar heights will be the provided
        /// values.
        struct BarChart {
            /// The values. Should always be a rank-1 tensor.
            rerun::components::TensorData values;

            /// Name of the indicator component, used to identify the archetype when converting to a
            /// list of components.
            static const char INDICATOR_COMPONENT_NAME[];
            using IndicatorComponent = components::IndicatorComponent<INDICATOR_COMPONENT_NAME>;

          public:
            BarChart() = default;
            BarChart(BarChart&& other) = default;

            BarChart(rerun::components::TensorData _values) : values(std::move(_values)) {}

            /// Returns the number of primary instances of this archetype.
            size_t num_instances() const {
                return 1;
            }

            /// TODO: move to trait
            Result<std::vector<SerializedComponentBatch>> serialize() const;
        };
    } // namespace archetypes
} // namespace rerun
