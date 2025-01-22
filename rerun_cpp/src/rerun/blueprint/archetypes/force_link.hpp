// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/force_link.fbs".

#pragma once

#include "../../blueprint/components/enabled.hpp"
#include "../../blueprint/components/force_distance.hpp"
#include "../../blueprint/components/force_iterations.hpp"
#include "../../collection.hpp"
#include "../../component_batch.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Aims to achieve a target distance between two nodes that are connected by an edge.
    struct ForceLink {
        /// Whether the link force is enabled.
        ///
        /// The link force aims to achieve a target distance between two nodes that are connected by one ore more edges.
        std::optional<ComponentBatch> enabled;

        /// The target distance between two nodes.
        std::optional<ComponentBatch> distance;

        /// Specifies how often this force should be applied per iteration.
        ///
        /// Increasing this parameter can lead to better results at the cost of longer computation time.
        std::optional<ComponentBatch> iterations;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.ForceLinkIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.ForceLink";

        /// `ComponentDescriptor` for the `enabled` field.
        static constexpr auto Descriptor_enabled = ComponentDescriptor(
            ArchetypeName, "enabled",
            Loggable<rerun::blueprint::components::Enabled>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `distance` field.
        static constexpr auto Descriptor_distance = ComponentDescriptor(
            ArchetypeName, "distance",
            Loggable<rerun::blueprint::components::ForceDistance>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `iterations` field.
        static constexpr auto Descriptor_iterations = ComponentDescriptor(
            ArchetypeName, "iterations",
            Loggable<rerun::blueprint::components::ForceIterations>::Descriptor.component_name
        );

      public:
        ForceLink() = default;
        ForceLink(ForceLink&& other) = default;
        ForceLink(const ForceLink& other) = default;
        ForceLink& operator=(const ForceLink& other) = default;
        ForceLink& operator=(ForceLink&& other) = default;

        /// Update only some specific fields of a `ForceLink`.
        static ForceLink update_fields() {
            return ForceLink();
        }

        /// Clear all the fields of a `ForceLink`.
        static ForceLink clear_fields();

        /// Whether the link force is enabled.
        ///
        /// The link force aims to achieve a target distance between two nodes that are connected by one ore more edges.
        ForceLink with_enabled(const rerun::blueprint::components::Enabled& _enabled) && {
            enabled = ComponentBatch::from_loggable(_enabled, Descriptor_enabled).value_or_throw();
            return std::move(*this);
        }

        /// The target distance between two nodes.
        ForceLink with_distance(const rerun::blueprint::components::ForceDistance& _distance) && {
            distance =
                ComponentBatch::from_loggable(_distance, Descriptor_distance).value_or_throw();
            return std::move(*this);
        }

        /// Specifies how often this force should be applied per iteration.
        ///
        /// Increasing this parameter can lead to better results at the cost of longer computation time.
        ForceLink with_iterations(const rerun::blueprint::components::ForceIterations& _iterations
        ) && {
            iterations =
                ComponentBatch::from_loggable(_iterations, Descriptor_iterations).value_or_throw();
            return std::move(*this);
        }
    };

} // namespace rerun::blueprint::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<blueprint::archetypes::ForceLink> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::ForceLink& archetype
        );
    };
} // namespace rerun
