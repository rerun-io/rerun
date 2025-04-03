// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/visualizer_overrides.fbs".

#pragma once

#include "../../blueprint/components/visualizer_override.hpp"
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
    /// **Archetype**: Override the visualizers for an entity.
    ///
    /// This archetype is a stop-gap mechanism based on the current implementation details
    /// of the visualizer system. It is not intended to be a long-term solution, but provides
    /// enough utility to be useful in the short term.
    ///
    /// The long-term solution is likely to be based off: <https://github.com/rerun-io/rerun/issues/6626>
    ///
    /// This can only be used as part of blueprints. It will have no effect if used
    /// in a regular entity.
    ///
    /// ⚠ **This type is _unstable_ and may change significantly in a way that the data won't be backwards compatible.**
    ///
    struct VisualizerOverrides {
        /// Names of the visualizers that should be active.
        std::optional<ComponentBatch> ranges;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.VisualizerOverridesIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] =
            "rerun.blueprint.archetypes.VisualizerOverrides";

        /// `ComponentDescriptor` for the `ranges` field.
        static constexpr auto Descriptor_ranges = ComponentDescriptor(
            ArchetypeName, "ranges",
            Loggable<rerun::blueprint::components::VisualizerOverride>::Descriptor.component_name
        );

      public:
        VisualizerOverrides() = default;
        VisualizerOverrides(VisualizerOverrides&& other) = default;
        VisualizerOverrides(const VisualizerOverrides& other) = default;
        VisualizerOverrides& operator=(const VisualizerOverrides& other) = default;
        VisualizerOverrides& operator=(VisualizerOverrides&& other) = default;

        explicit VisualizerOverrides(
            Collection<rerun::blueprint::components::VisualizerOverride> _ranges
        )
            : ranges(ComponentBatch::from_loggable(std::move(_ranges), Descriptor_ranges)
                         .value_or_throw()) {}

        /// Update only some specific fields of a `VisualizerOverrides`.
        static VisualizerOverrides update_fields() {
            return VisualizerOverrides();
        }

        /// Clear all the fields of a `VisualizerOverrides`.
        static VisualizerOverrides clear_fields();

        /// Names of the visualizers that should be active.
        VisualizerOverrides with_ranges(
            const Collection<rerun::blueprint::components::VisualizerOverride>& _ranges
        ) && {
            ranges = ComponentBatch::from_loggable(_ranges, Descriptor_ranges).value_or_throw();
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
    struct AsComponents<blueprint::archetypes::VisualizerOverrides> {
        /// Serialize all set component batches.
        static Result<Collection<ComponentBatch>> as_batches(
            const blueprint::archetypes::VisualizerOverrides& archetype
        );
    };
} // namespace rerun
