// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/visible_time_ranges.fbs".

#pragma once

#include "../../blueprint/components/visible_time_range.hpp"
#include "../../collection.hpp"
#include "../../component_batch.hpp"
#include "../../component_column.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Configures what range of each timeline is shown on a view.
    ///
    /// Whenever no visual time range applies, queries are done with "latest-at" semantics.
    /// This means that the view will, starting from the time cursor position,
    /// query the latest data available for each component type.
    ///
    /// The default visual time range depends on the type of view this property applies to:
    /// - For time series views, the default is to show the entire timeline.
    /// - For any other view, the default is to apply latest-at semantics.
    ///
    /// ⚠ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
    ///
    struct VisibleTimeRanges {
        /// The time ranges to show for each timeline unless specified otherwise on a per-entity basis.
        ///
        /// If a timeline is specified more than once, the first entry will be used.
        std::optional<ComponentBatch> ranges;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.VisibleTimeRangesIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] =
            "rerun.blueprint.archetypes.VisibleTimeRanges";

        /// `ComponentDescriptor` for the `ranges` field.
        static constexpr auto Descriptor_ranges = ComponentDescriptor(
            ArchetypeName, "ranges",
            Loggable<rerun::blueprint::components::VisibleTimeRange>::Descriptor.component_name
        );

      public:
        VisibleTimeRanges() = default;
        VisibleTimeRanges(VisibleTimeRanges&& other) = default;
        VisibleTimeRanges(const VisibleTimeRanges& other) = default;
        VisibleTimeRanges& operator=(const VisibleTimeRanges& other) = default;
        VisibleTimeRanges& operator=(VisibleTimeRanges&& other) = default;

        explicit VisibleTimeRanges(
            Collection<rerun::blueprint::components::VisibleTimeRange> _ranges
        )
            : ranges(ComponentBatch::from_loggable(std::move(_ranges), Descriptor_ranges)
                         .value_or_throw()) {}

        /// Update only some specific fields of a `VisibleTimeRanges`.
        static VisibleTimeRanges update_fields() {
            return VisibleTimeRanges();
        }

        /// Clear all the fields of a `VisibleTimeRanges`.
        static VisibleTimeRanges clear_fields();

        /// The time ranges to show for each timeline unless specified otherwise on a per-entity basis.
        ///
        /// If a timeline is specified more than once, the first entry will be used.
        VisibleTimeRanges with_ranges(
            const Collection<rerun::blueprint::components::VisibleTimeRange>& _ranges
        ) && {
            ranges = ComponentBatch::from_loggable(_ranges, Descriptor_ranges).value_or_throw();
            return std::move(*this);
        }

        /// Partitions the component data into multiple sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into `ComponentColumn`s
        /// instead, via `ComponentBatch::partitioned`.
        ///
        /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
        ///
        /// The specified `lengths` must sum to the total length of the component batch.
        Collection<ComponentColumn> columns(const Collection<uint32_t>& lengths_);

        /// Partitions the component data into unit-length sub-batches.
        ///
        /// This is semantically similar to calling `columns` with `std::vector<uint32_t>(n, 1)`,
        /// where `n` is automatically guessed.
        Collection<ComponentColumn> columns();
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
        static Result<Collection<ComponentBatch>> as_batches(
            const blueprint::archetypes::VisibleTimeRanges& archetype
        );
    };
} // namespace rerun
