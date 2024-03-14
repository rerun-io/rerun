// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/archetypes/space_view_contents.fbs".

#pragma once

#include "../../blueprint/components/query_expression.hpp"
#include "../../collection.hpp"
#include "../../data_cell.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: The contents of a `SpaceView`.
    ///
    /// The contents are found by combining a collection of `QueryExpression`s.
    ///
    /// ```diff
    /// + /world/**           # add everything…
    /// - /world/roads/**     # …but remove all roads…
    /// + /world/roads/main   # …but show main road
    /// ```
    ///
    /// If there is multiple matching rules, the most specific rule wins.
    /// If there are multiple rules of the same specificity, the last one wins.
    /// If no rules match, the path is excluded.
    ///
    /// The `/**` suffix matches the whole subtree, i.e. self and any child, recursively
    /// (`/world/**` matches both `/world` and `/world/car/driver`).
    /// Other uses of `*` are not (yet) supported.
    ///
    /// Internally, `EntityPathFilter` sorts the rule by entity path, with recursive coming before non-recursive.
    /// This means the last matching rule is also the most specific one.  For instance:
    /// ```diff
    /// + /world/**
    /// - /world
    /// - /world/car/**
    /// + /world/car/driver
    /// ```
    ///
    /// The last rule matching `/world/car/driver` is `+ /world/car/driver`, so it is included.
    /// The last rule matching `/world/car/hood` is `- /world/car/**`, so it is excluded.
    /// The last rule matching `/world` is `- /world`, so it is excluded.
    /// The last rule matching `/world/house` is `+ /world/**`, so it is included.
    ///
    /// Unstable. Used for the ongoing blueprint experimentations.
    struct SpaceViewContents {
        /// The `QueryExpression` that populates the contents for the `SpaceView`.
        ///
        /// They determine which entities are part of the spaceview.
        Collection<rerun::blueprint::components::QueryExpression> query;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.SpaceViewContentsIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        SpaceViewContents() = default;
        SpaceViewContents(SpaceViewContents&& other) = default;

        explicit SpaceViewContents(Collection<rerun::blueprint::components::QueryExpression> _query)
            : query(std::move(_query)) {}

        /// Returns the number of primary instances of this archetype.
        size_t num_instances() const {
            return query.size();
        }
    };

} // namespace rerun::blueprint::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<blueprint::archetypes::SpaceViewContents> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(
            const blueprint::archetypes::SpaceViewContents& archetype
        );
    };
} // namespace rerun
