// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/force_many_body.fbs".

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
    /// **Archetype**: A force between each pair of nodes that ressembles an electrical charge.
    ///
    /// If `strength` is smaller than 0, it pushes nodes apart, if it is larger than 0 it pulls them together.
    struct ForceManyBody {
        /// Whether the many body force is enabled.
        ///
        /// The many body force is applied on each pair of nodes in a way that ressembles an electrical charge. If the
        /// strength is smaller than 0, it pushes nodes apart; if it is larger than 0, it pulls them together.
        std::optional<ComponentBatch> enabled;

        /// The strength of the force.
        ///
        /// If `strength` is smaller than 0, it pushes nodes apart, if it is larger than 0 it pulls them together.
        std::optional<ComponentBatch> strength;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.ForceManyBodyIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.ForceManyBody";

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
        ForceManyBody() = default;
        ForceManyBody(ForceManyBody&& other) = default;
        ForceManyBody(const ForceManyBody& other) = default;
        ForceManyBody& operator=(const ForceManyBody& other) = default;
        ForceManyBody& operator=(ForceManyBody&& other) = default;

        /// Update only some specific fields of a `ForceManyBody`.
        static ForceManyBody update_fields() {
            return ForceManyBody();
        }

        /// Clear all the fields of a `ForceManyBody`.
        static ForceManyBody clear_fields();

        /// Whether the many body force is enabled.
        ///
        /// The many body force is applied on each pair of nodes in a way that ressembles an electrical charge. If the
        /// strength is smaller than 0, it pushes nodes apart; if it is larger than 0, it pulls them together.
        ForceManyBody with_enabled(const rerun::blueprint::components::Enabled& _enabled) && {
            enabled = ComponentBatch::from_loggable(_enabled, Descriptor_enabled).value_or_throw();
            return std::move(*this);
        }

        /// The strength of the force.
        ///
        /// If `strength` is smaller than 0, it pushes nodes apart, if it is larger than 0 it pulls them together.
        ForceManyBody with_strength(const rerun::blueprint::components::ForceStrength& _strength
        ) && {
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
    struct AsComponents<blueprint::archetypes::ForceManyBody> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::ForceManyBody& archetype
        );
    };
} // namespace rerun
