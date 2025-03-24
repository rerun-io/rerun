// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/entity_behavior.fbs".

#pragma once

#include "../../collection.hpp"
#include "../../component_batch.hpp"
#include "../../component_column.hpp"
#include "../../components/interactive.hpp"
#include "../../components/visible.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: General visualization behavior of an entity.
    ///
    /// TODO(#6541): Fields of this archetype currently only have an effect when logged in the blueprint store.
    struct EntityBehavior {
        /// Whether the entity can be interacted with.
        ///
        /// This property is propagated down the entity hierarchy until another child entity
        /// sets `interactive` to a different value at which point propagation continues with that value instead.
        ///
        /// Defaults to parent's `interactive` value or true if there is no parent.
        std::optional<ComponentBatch> interactive;

        /// Whether the entity is visible.
        ///
        /// This property is propagated down the entity hierarchy until another child entity
        /// sets `visible` to a different value at which point propagation continues with that value instead.
        ///
        /// Defaults to parent's `visible` value or true if there is no parent.
        std::optional<ComponentBatch> visible;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.EntityBehaviorIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.EntityBehavior";

        /// `ComponentDescriptor` for the `interactive` field.
        static constexpr auto Descriptor_interactive = ComponentDescriptor(
            ArchetypeName, "interactive",
            Loggable<rerun::components::Interactive>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `visible` field.
        static constexpr auto Descriptor_visible = ComponentDescriptor(
            ArchetypeName, "visible",
            Loggable<rerun::components::Visible>::Descriptor.component_name
        );

      public:
        EntityBehavior() = default;
        EntityBehavior(EntityBehavior&& other) = default;
        EntityBehavior(const EntityBehavior& other) = default;
        EntityBehavior& operator=(const EntityBehavior& other) = default;
        EntityBehavior& operator=(EntityBehavior&& other) = default;

        /// Update only some specific fields of a `EntityBehavior`.
        static EntityBehavior update_fields() {
            return EntityBehavior();
        }

        /// Clear all the fields of a `EntityBehavior`.
        static EntityBehavior clear_fields();

        /// Whether the entity can be interacted with.
        ///
        /// This property is propagated down the entity hierarchy until another child entity
        /// sets `interactive` to a different value at which point propagation continues with that value instead.
        ///
        /// Defaults to parent's `interactive` value or true if there is no parent.
        EntityBehavior with_interactive(const rerun::components::Interactive& _interactive) && {
            interactive = ComponentBatch::from_loggable(_interactive, Descriptor_interactive)
                              .value_or_throw();
            return std::move(*this);
        }

        /// Whether the entity is visible.
        ///
        /// This property is propagated down the entity hierarchy until another child entity
        /// sets `visible` to a different value at which point propagation continues with that value instead.
        ///
        /// Defaults to parent's `visible` value or true if there is no parent.
        EntityBehavior with_visible(const rerun::components::Visible& _visible) && {
            visible = ComponentBatch::from_loggable(_visible, Descriptor_visible).value_or_throw();
            return std::move(*this);
        }

        /// Partitions the component data into multiple sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into `ComponentColumn`s
        /// instead, via `ComponentBatch::partitioned`.
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
    struct AsComponents<blueprint::archetypes::EntityBehavior> {
        /// Serialize all set component batches.
        static Result<Collection<ComponentBatch>> as_batches(
            const blueprint::archetypes::EntityBehavior& archetype
        );
    };
} // namespace rerun
