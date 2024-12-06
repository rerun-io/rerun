// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/force_collision_radius.fbs".

#pragma once

#include "../../blueprint/components/enabled.hpp"
#include "../../blueprint/components/force_strength.hpp"
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
    struct ForceCollisionRadius {
        /// Whether the force is enabled.
        std::optional<rerun::blueprint::components::Enabled> enabled;

        /// The strength of the force.
        std::optional<rerun::blueprint::components::ForceStrength> strength;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.ForceCollisionRadiusIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        ForceCollisionRadius() = default;
        ForceCollisionRadius(ForceCollisionRadius&& other) = default;

        /// Whether the force is enabled.
        ForceCollisionRadius with_enabled(rerun::blueprint::components::Enabled _enabled) && {
            enabled = std::move(_enabled);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// The strength of the force.
        ForceCollisionRadius with_strength(rerun::blueprint::components::ForceStrength _strength
        ) && {
            strength = std::move(_strength);
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
    struct AsComponents<blueprint::archetypes::ForceCollisionRadius> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::ForceCollisionRadius& archetype
        );
    };
} // namespace rerun
