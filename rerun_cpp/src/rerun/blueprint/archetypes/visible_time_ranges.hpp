// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/visible_time_ranges.fbs".

#pragma once

#include "../../blueprint/components/visible_time_range.hpp"
#include "../../collection.hpp"
#include "../../data_cell.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Configures what range of each timeline is shown on a view.
    ///
    /// Whenever no visual time range applies, queries are done with "latest at" semantics.
    /// This means that the view will, starting from the time cursor position,
    /// query the latest data available for each component type.
    ///
    /// The default visual time range depends on the type of view this property applies to:
    /// - For time series views, the default is to show the entire timeline.
    /// - For any other view, the default is to apply latest-at semantics.
    struct VisibleTimeRanges {
        /// The time ranges to show for each timeline unless specified otherwise on a per-entity basis.
        ///
        /// If a timeline is specified more than once, the first entry will be used.
        Collection<rerun::blueprint::components::VisibleTimeRange> ranges;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.VisibleTimeRangesIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        VisibleTimeRanges() = default;
        VisibleTimeRanges(VisibleTimeRanges&& other) = default;

        explicit VisibleTimeRanges(
            Collection<rerun::blueprint::components::VisibleTimeRange> _ranges
        )
            : ranges(std::move(_ranges)) {}
    };

} // namespace rerun::blueprint::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<blueprint::archetypes::VisibleTimeRanges> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(
            const blueprint::archetypes::VisibleTimeRanges& archetype
        );
    };
} // namespace rerun
