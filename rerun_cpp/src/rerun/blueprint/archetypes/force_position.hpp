// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/force_position.fbs".

#pragma once

#include "../../blueprint/components/enabled.hpp"
#include "../../blueprint/components/force_strength.hpp"
#include "../../collection.hpp"
#include "../../component_batch.hpp"
#include "../../components/position2d.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Similar to gravity, this force pulls nodes towards a specific position.
    struct ForcePosition {
        /// Whether the position force is enabled.
        ///
        /// The position force pulls nodes towards a specific position, similar to gravity.
        std::optional<ComponentBatch> enabled;

        /// The strength of the force.
        std::optional<ComponentBatch> strength;

        /// The position where the nodes should be pulled towards.
        std::optional<ComponentBatch> position;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.ForcePositionIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.ForcePosition";

        /// `ComponentDescriptor` for the `enabled` field.
        static constexpr auto Descriptor_enabled = ComponentDescriptor(
            ArchetypeName, "enabled",
            Loggable<rerun::blueprint::components::Enabled>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `strength` field.
        static constexpr auto Descriptor_strength = ComponentDescriptor(
            ArchetypeName, "strength",
            Loggable<rerun::blueprint::components::ForceStrength>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `position` field.
        static constexpr auto Descriptor_position = ComponentDescriptor(
            ArchetypeName, "position",
            Loggable<rerun::components::Position2D>::Descriptor.component_name
        );

      public:
        ForcePosition() = default;
        ForcePosition(ForcePosition&& other) = default;
        ForcePosition(const ForcePosition& other) = default;
        ForcePosition& operator=(const ForcePosition& other) = default;
        ForcePosition& operator=(ForcePosition&& other) = default;

        /// Update only some specific fields of a `ForcePosition`.
        static ForcePosition update_fields() {
            return ForcePosition();
        }

        /// Clear all the fields of a `ForcePosition`.
        static ForcePosition clear_fields();

        /// Whether the position force is enabled.
        ///
        /// The position force pulls nodes towards a specific position, similar to gravity.
        ForcePosition with_enabled(const rerun::blueprint::components::Enabled& _enabled) && {
            enabled = ComponentBatch::from_loggable(_enabled, Descriptor_enabled).value_or_throw();
            return std::move(*this);
        }

        /// The strength of the force.
        ForcePosition with_strength(const rerun::blueprint::components::ForceStrength& _strength
        ) && {
            strength =
                ComponentBatch::from_loggable(_strength, Descriptor_strength).value_or_throw();
            return std::move(*this);
        }

        /// The position where the nodes should be pulled towards.
        ForcePosition with_position(const rerun::components::Position2D& _position) && {
            position =
                ComponentBatch::from_loggable(_position, Descriptor_position).value_or_throw();
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
    struct AsComponents<blueprint::archetypes::ForcePosition> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::ForcePosition& archetype
        );
    };
} // namespace rerun
