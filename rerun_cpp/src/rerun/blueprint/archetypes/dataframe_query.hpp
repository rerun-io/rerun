// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/dataframe_query.fbs".

#pragma once

#include "../../blueprint/components/apply_latest_at.hpp"
#include "../../blueprint/components/filter_by_range.hpp"
#include "../../blueprint/components/filter_is_not_null.hpp"
#include "../../blueprint/components/selected_columns.hpp"
#include "../../blueprint/components/timeline_name.hpp"
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
    /// **Archetype**: The query for the dataframe view.
    ///
    /// ⚠ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
    ///
    struct DataframeQuery {
        /// The timeline for this query.
        ///
        /// If unset, the timeline currently active on the time panel is used.
        std::optional<ComponentBatch> timeline;

        /// If provided, only rows whose timestamp is within this range will be shown.
        ///
        /// Note: will be unset as soon as `timeline` is changed.
        std::optional<ComponentBatch> filter_by_range;

        /// If provided, only show rows which contains a logged event for the specified component.
        std::optional<ComponentBatch> filter_is_not_null;

        /// Should empty cells be filled with latest-at queries?
        std::optional<ComponentBatch> apply_latest_at;

        /// Selected columns. If unset, all columns are selected.
        std::optional<ComponentBatch> select;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.DataframeQueryIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.DataframeQuery";

        /// `ComponentDescriptor` for the `timeline` field.
        static constexpr auto Descriptor_timeline = ComponentDescriptor(
            ArchetypeName, "timeline",
            Loggable<rerun::blueprint::components::TimelineName>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `filter_by_range` field.
        static constexpr auto Descriptor_filter_by_range = ComponentDescriptor(
            ArchetypeName, "filter_by_range",
            Loggable<rerun::blueprint::components::FilterByRange>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `filter_is_not_null` field.
        static constexpr auto Descriptor_filter_is_not_null = ComponentDescriptor(
            ArchetypeName, "filter_is_not_null",
            Loggable<rerun::blueprint::components::FilterIsNotNull>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `apply_latest_at` field.
        static constexpr auto Descriptor_apply_latest_at = ComponentDescriptor(
            ArchetypeName, "apply_latest_at",
            Loggable<rerun::blueprint::components::ApplyLatestAt>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `select` field.
        static constexpr auto Descriptor_select = ComponentDescriptor(
            ArchetypeName, "select",
            Loggable<rerun::blueprint::components::SelectedColumns>::Descriptor.component_name
        );

      public:
        DataframeQuery() = default;
        DataframeQuery(DataframeQuery&& other) = default;
        DataframeQuery(const DataframeQuery& other) = default;
        DataframeQuery& operator=(const DataframeQuery& other) = default;
        DataframeQuery& operator=(DataframeQuery&& other) = default;

        /// Update only some specific fields of a `DataframeQuery`.
        static DataframeQuery update_fields() {
            return DataframeQuery();
        }

        /// Clear all the fields of a `DataframeQuery`.
        static DataframeQuery clear_fields();

        /// The timeline for this query.
        ///
        /// If unset, the timeline currently active on the time panel is used.
        DataframeQuery with_timeline(const rerun::blueprint::components::TimelineName& _timeline
        ) && {
            timeline =
                ComponentBatch::from_loggable(_timeline, Descriptor_timeline).value_or_throw();
            return std::move(*this);
        }

        /// If provided, only rows whose timestamp is within this range will be shown.
        ///
        /// Note: will be unset as soon as `timeline` is changed.
        DataframeQuery with_filter_by_range(
            const rerun::blueprint::components::FilterByRange& _filter_by_range
        ) && {
            filter_by_range =
                ComponentBatch::from_loggable(_filter_by_range, Descriptor_filter_by_range)
                    .value_or_throw();
            return std::move(*this);
        }

        /// If provided, only show rows which contains a logged event for the specified component.
        DataframeQuery with_filter_is_not_null(
            const rerun::blueprint::components::FilterIsNotNull& _filter_is_not_null
        ) && {
            filter_is_not_null =
                ComponentBatch::from_loggable(_filter_is_not_null, Descriptor_filter_is_not_null)
                    .value_or_throw();
            return std::move(*this);
        }

        /// Should empty cells be filled with latest-at queries?
        DataframeQuery with_apply_latest_at(
            const rerun::blueprint::components::ApplyLatestAt& _apply_latest_at
        ) && {
            apply_latest_at =
                ComponentBatch::from_loggable(_apply_latest_at, Descriptor_apply_latest_at)
                    .value_or_throw();
            return std::move(*this);
        }

        /// Selected columns. If unset, all columns are selected.
        DataframeQuery with_select(const rerun::blueprint::components::SelectedColumns& _select
        ) && {
            select = ComponentBatch::from_loggable(_select, Descriptor_select).value_or_throw();
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
    struct AsComponents<blueprint::archetypes::DataframeQuery> {
        /// Serialize all set component batches.
        static Result<Collection<ComponentBatch>> as_batches(
            const blueprint::archetypes::DataframeQuery& archetype
        );
    };
} // namespace rerun
