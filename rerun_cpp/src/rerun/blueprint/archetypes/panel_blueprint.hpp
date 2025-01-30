// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/panel_blueprint.fbs".

#pragma once

#include "../../blueprint/components/panel_state.hpp"
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
    /// **Archetype**: Shared state for the 3 collapsible panels.
    struct PanelBlueprint {
        /// Current state of the panels.
        std::optional<ComponentBatch> state;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.PanelBlueprintIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.PanelBlueprint";

        /// `ComponentDescriptor` for the `state` field.
        static constexpr auto Descriptor_state = ComponentDescriptor(
            ArchetypeName, "state",
            Loggable<rerun::blueprint::components::PanelState>::Descriptor.component_name
        );

      public:
        PanelBlueprint() = default;
        PanelBlueprint(PanelBlueprint&& other) = default;
        PanelBlueprint(const PanelBlueprint& other) = default;
        PanelBlueprint& operator=(const PanelBlueprint& other) = default;
        PanelBlueprint& operator=(PanelBlueprint&& other) = default;

        /// Update only some specific fields of a `PanelBlueprint`.
        static PanelBlueprint update_fields() {
            return PanelBlueprint();
        }

        /// Clear all the fields of a `PanelBlueprint`.
        static PanelBlueprint clear_fields();

        /// Current state of the panels.
        PanelBlueprint with_state(const rerun::blueprint::components::PanelState& _state) && {
            state = ComponentBatch::from_loggable(_state, Descriptor_state).value_or_throw();
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
    struct AsComponents<blueprint::archetypes::PanelBlueprint> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::PanelBlueprint& archetype
        );
    };
} // namespace rerun
