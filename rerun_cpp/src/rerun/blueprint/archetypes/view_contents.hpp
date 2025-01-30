// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/view_contents.fbs".

#pragma once

#include "../../blueprint/components/query_expression.hpp"
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
    /// **Archetype**: The contents of a `View`.
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
    /// Specifying a path without a `+` or `-` prefix is equivalent to `+`:
    /// ```diff
    /// /world/**           # add everything…
    /// - /world/roads/**   # …but remove all roads…
    /// /world/roads/main   # …but show main road
    /// ```
    ///
    /// The `/**` suffix matches the whole subtree, i.e. self and any child, recursively
    /// (`/world/**` matches both `/world` and `/world/car/driver`).
    /// Other uses of `*` are not (yet) supported.
    ///
    /// Internally, `EntityPathFilter` sorts the rule by entity path, with recursive coming before non-recursive.
    /// This means the last matching rule is also the most specific one. For instance:
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
    struct ViewContents {
        /// The `QueryExpression` that populates the contents for the view.
        ///
        /// They determine which entities are part of the view.
        std::optional<ComponentBatch> query;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.ViewContentsIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.ViewContents";

        /// `ComponentDescriptor` for the `query` field.
        static constexpr auto Descriptor_query = ComponentDescriptor(
            ArchetypeName, "query",
            Loggable<rerun::blueprint::components::QueryExpression>::Descriptor.component_name
        );

      public:
        ViewContents() = default;
        ViewContents(ViewContents&& other) = default;
        ViewContents(const ViewContents& other) = default;
        ViewContents& operator=(const ViewContents& other) = default;
        ViewContents& operator=(ViewContents&& other) = default;

        explicit ViewContents(Collection<rerun::blueprint::components::QueryExpression> _query)
            : query(ComponentBatch::from_loggable(std::move(_query), Descriptor_query)
                        .value_or_throw()) {}

        /// Update only some specific fields of a `ViewContents`.
        static ViewContents update_fields() {
            return ViewContents();
        }

        /// Clear all the fields of a `ViewContents`.
        static ViewContents clear_fields();

        /// The `QueryExpression` that populates the contents for the view.
        ///
        /// They determine which entities are part of the view.
        ViewContents with_query(
            const Collection<rerun::blueprint::components::QueryExpression>& _query
        ) && {
            query = ComponentBatch::from_loggable(_query, Descriptor_query).value_or_throw();
            return std::move(*this);
        }

        /// Partitions the component data into multiple sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into `ComponentColumn`s
        /// instead, via `ComponentColumn::from_batch_with_lengths`.
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
    struct AsComponents<blueprint::archetypes::ViewContents> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::ViewContents& archetype
        );
    };
} // namespace rerun
