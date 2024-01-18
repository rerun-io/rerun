// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/archetypes/time_series.fbs".

#pragma once

#include "../../blueprint/components/legend.hpp"
#include "../../collection.hpp"
#include "../../data_cell.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: The top-level description of the Viewport.
    struct TimeSeries {
        /// Configuration information for the legend
        rerun::blueprint::components::Legend legend;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.TimeSeriesIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        TimeSeries() = default;
        TimeSeries(TimeSeries&& other) = default;

        explicit TimeSeries(rerun::blueprint::components::Legend _legend)
            : legend(std::move(_legend)) {}

        /// Returns the number of primary instances of this archetype.
        size_t num_instances() const {
            return 1;
        }
    };

} // namespace rerun::blueprint::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<blueprint::archetypes::TimeSeries> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(
            const blueprint::archetypes::TimeSeries& archetype
        );
    };
} // namespace rerun
