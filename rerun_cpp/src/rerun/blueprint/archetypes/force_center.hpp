// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/force_center.fbs".

#pragma once

#include "../../blueprint/components/enabled.hpp"
#include "../../blueprint/components/force_strength.hpp"
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
    /// **Archetype**: Tries to move the center of mass of the graph to the origin.
    struct ForceCenter {
        /// Whether the center force is enabled.
        ///
        /// The center force tries to move the center of mass of the graph towards the origin.
        std::optional<ComponentBatch> enabled;

        /// The strength of the force.
        std::optional<ComponentBatch> strength;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.ForceCenterIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.ForceCenter";

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

      public:
        ForceCenter() = default;
        ForceCenter(ForceCenter&& other) = default;
        ForceCenter(const ForceCenter& other) = default;
        ForceCenter& operator=(const ForceCenter& other) = default;
        ForceCenter& operator=(ForceCenter&& other) = default;

        /// Update only some specific fields of a `ForceCenter`.
        static ForceCenter update_fields() {
            return ForceCenter();
        }

        /// Clear all the fields of a `ForceCenter`.
        static ForceCenter clear_fields();

        /// Whether the center force is enabled.
        ///
        /// The center force tries to move the center of mass of the graph towards the origin.
        ForceCenter with_enabled(const rerun::blueprint::components::Enabled& _enabled) && {
            enabled = ComponentBatch::from_loggable(_enabled, Descriptor_enabled).value_or_throw();
            return std::move(*this);
        }

        /// The strength of the force.
        ForceCenter with_strength(const rerun::blueprint::components::ForceStrength& _strength) && {
            strength =
                ComponentBatch::from_loggable(_strength, Descriptor_strength).value_or_throw();
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
    struct AsComponents<blueprint::archetypes::ForceCenter> {
        /// Serialize all set component batches.
        static Result<Collection<ComponentBatch>> as_batches(
            const blueprint::archetypes::ForceCenter& archetype
        );
    };
} // namespace rerun
