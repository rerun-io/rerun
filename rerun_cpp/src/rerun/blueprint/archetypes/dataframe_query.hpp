// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/dataframe_query.fbs".

#pragma once

#include "../../blueprint/components/latest_at_queries.hpp"
#include "../../blueprint/components/query_kind.hpp"
#include "../../blueprint/components/time_range_queries.hpp"
#include "../../blueprint/components/timeline_name.hpp"
#include "../../collection.hpp"
#include "../../compiler_utils.hpp"
#include "../../component_batch.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: The query for the dataframe view.
    struct DataframeQuery {
        /// The timeline for this query.
        ///
        /// If unset, use the time panel's timeline and a latest at query, ignoring all other components of this archetype.
        std::optional<rerun::blueprint::components::TimelineName> timeline;

        /// Kind of query: latest-at or range.
        std::optional<rerun::blueprint::components::QueryKind> kind;

        /// Configuration for latest-at queries.
        ///
        /// Note: configuration as saved on a per-timeline basis.
        std::optional<rerun::blueprint::components::LatestAtQueries> latest_at_queries;

        /// Configuration for the time range queries.
        ///
        /// Note: configuration as saved on a per-timeline basis.
        std::optional<rerun::blueprint::components::TimeRangeQueries> time_range_queries;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.DataframeQueryIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        DataframeQuery() = default;
        DataframeQuery(DataframeQuery&& other) = default;

        /// The timeline for this query.
        ///
        /// If unset, use the time panel's timeline and a latest at query, ignoring all other components of this archetype.
        DataframeQuery with_timeline(rerun::blueprint::components::TimelineName _timeline) && {
            timeline = std::move(_timeline);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Kind of query: latest-at or range.
        DataframeQuery with_kind(rerun::blueprint::components::QueryKind _kind) && {
            kind = std::move(_kind);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Configuration for latest-at queries.
        ///
        /// Note: configuration as saved on a per-timeline basis.
        DataframeQuery with_latest_at_queries(
            rerun::blueprint::components::LatestAtQueries _latest_at_queries
        ) && {
            latest_at_queries = std::move(_latest_at_queries);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Configuration for the time range queries.
        ///
        /// Note: configuration as saved on a per-timeline basis.
        DataframeQuery with_time_range_queries(
            rerun::blueprint::components::TimeRangeQueries _time_range_queries
        ) && {
            time_range_queries = std::move(_time_range_queries);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }
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
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::DataframeQuery& archetype
        );
    };
} // namespace rerun
