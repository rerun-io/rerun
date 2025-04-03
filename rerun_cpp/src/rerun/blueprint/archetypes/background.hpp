// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/background.fbs".

#pragma once

#include "../../blueprint/components/background_kind.hpp"
#include "../../collection.hpp"
#include "../../component_batch.hpp"
#include "../../component_column.hpp"
#include "../../components/color.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Configuration for the background of a view.
    ///
    /// ⚠ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
    ///
    struct Background {
        /// The type of the background.
        std::optional<ComponentBatch> kind;

        /// Color used for the solid background type.
        std::optional<ComponentBatch> color;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.BackgroundIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.Background";

        /// `ComponentDescriptor` for the `kind` field.
        static constexpr auto Descriptor_kind = ComponentDescriptor(
            ArchetypeName, "kind",
            Loggable<rerun::blueprint::components::BackgroundKind>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `color` field.
        static constexpr auto Descriptor_color = ComponentDescriptor(
            ArchetypeName, "color", Loggable<rerun::components::Color>::Descriptor.component_name
        );

      public:
        Background() = default;
        Background(Background&& other) = default;
        Background(const Background& other) = default;
        Background& operator=(const Background& other) = default;
        Background& operator=(Background&& other) = default;

        explicit Background(rerun::blueprint::components::BackgroundKind _kind)
            : kind(ComponentBatch::from_loggable(std::move(_kind), Descriptor_kind).value_or_throw()
              ) {}

        /// Update only some specific fields of a `Background`.
        static Background update_fields() {
            return Background();
        }

        /// Clear all the fields of a `Background`.
        static Background clear_fields();

        /// The type of the background.
        Background with_kind(const rerun::blueprint::components::BackgroundKind& _kind) && {
            kind = ComponentBatch::from_loggable(_kind, Descriptor_kind).value_or_throw();
            return std::move(*this);
        }

        /// Color used for the solid background type.
        Background with_color(const rerun::components::Color& _color) && {
            color = ComponentBatch::from_loggable(_color, Descriptor_color).value_or_throw();
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
    struct AsComponents<blueprint::archetypes::Background> {
        /// Serialize all set component batches.
        static Result<Collection<ComponentBatch>> as_batches(
            const blueprint::archetypes::Background& archetype
        );
    };
} // namespace rerun
