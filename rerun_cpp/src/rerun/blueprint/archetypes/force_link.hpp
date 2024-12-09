// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/force_link.fbs".

#pragma once

#include "../../blueprint/components/enabled.hpp"
#include "../../blueprint/components/force_distance.hpp"
#include "../../blueprint/components/force_iterations.hpp"
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
    /// **Archetype**: The link force pushes linked nodes together or apart according to a desired distance.
    struct ForceLink {
        /// Whether the force is enabled.
        std::optional<rerun::blueprint::components::Enabled> enabled;

        /// The target distance between two nodes.
        std::optional<rerun::blueprint::components::ForceDistance> distance;

        /// The number of iterations to run the force.
        std::optional<rerun::blueprint::components::ForceIterations> iterations;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.ForceLinkIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        ForceLink() = default;
        ForceLink(ForceLink&& other) = default;

        /// Whether the force is enabled.
        ForceLink with_enabled(rerun::blueprint::components::Enabled _enabled) && {
            enabled = std::move(_enabled);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// The target distance between two nodes.
        ForceLink with_distance(rerun::blueprint::components::ForceDistance _distance) && {
            distance = std::move(_distance);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// The number of iterations to run the force.
        ForceLink with_iterations(rerun::blueprint::components::ForceIterations _iterations) && {
            iterations = std::move(_iterations);
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
    struct AsComponents<blueprint::archetypes::ForceLink> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::ForceLink& archetype
        );
    };
} // namespace rerun
