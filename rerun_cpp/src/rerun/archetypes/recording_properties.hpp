// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/recording_properties.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
#include "../components/recording_name.hpp"
#include "../components/recording_started_timestamp.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: A list of properties associated with a recording.
    ///
    /// ## Example
    ///
    /// ### Simple directed graph
    /// ![image](https://static.rerun.io/graph_directed/ca29a37b65e1e0b6482251dce401982a0bc568fa/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_graph_directed");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     rec.log(
    ///         "simple",
    ///         rerun::GraphNodes({"a", "b", "c"})
    ///             .with_positions({{0.0, 100.0}, {-100.0, 0.0}, {100.0, 0.0}})
    ///             .with_labels({"A", "B", "C"}),
    ///         rerun::GraphEdges({{"a", "b"}, {"b", "c"}, {"c", "a"}})
    ///             // Graphs are undirected by default.
    ///             .with_graph_type(rerun::components::GraphType::Directed)
    ///     );
    /// }
    /// ```
    struct RecordingProperties {
        /// When the recording started.
        ///
        /// Should be an absolute time, i.e. relative to Unix Epoch.
        std::optional<ComponentBatch> started;

        /// A user-chosen name for the recording.
        std::optional<ComponentBatch> name;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.RecordingPropertiesIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.RecordingProperties";

        /// `ComponentDescriptor` for the `started` field.
        static constexpr auto Descriptor_started = ComponentDescriptor(
            ArchetypeName, "started",
            Loggable<rerun::components::RecordingStartedTimestamp>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `name` field.
        static constexpr auto Descriptor_name = ComponentDescriptor(
            ArchetypeName, "name",
            Loggable<rerun::components::RecordingName>::Descriptor.component_name
        );

      public:
        RecordingProperties() = default;
        RecordingProperties(RecordingProperties&& other) = default;
        RecordingProperties(const RecordingProperties& other) = default;
        RecordingProperties& operator=(const RecordingProperties& other) = default;
        RecordingProperties& operator=(RecordingProperties&& other) = default;

        explicit RecordingProperties(
            Collection<rerun::components::RecordingStartedTimestamp> _started
        )
            : started(ComponentBatch::from_loggable(std::move(_started), Descriptor_started)
                          .value_or_throw()) {}

        /// Update only some specific fields of a `RecordingProperties`.
        static RecordingProperties update_fields() {
            return RecordingProperties();
        }

        /// Clear all the fields of a `RecordingProperties`.
        static RecordingProperties clear_fields();

        /// When the recording started.
        ///
        /// Should be an absolute time, i.e. relative to Unix Epoch.
        RecordingProperties with_started(
            const Collection<rerun::components::RecordingStartedTimestamp>& _started
        ) && {
            started = ComponentBatch::from_loggable(_started, Descriptor_started).value_or_throw();
            return std::move(*this);
        }

        /// A user-chosen name for the recording.
        RecordingProperties with_name(const Collection<rerun::components::RecordingName>& _name
        ) && {
            name = ComponentBatch::from_loggable(_name, Descriptor_name).value_or_throw();
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

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::RecordingProperties> {
        /// Serialize all set component batches.
        static Result<Collection<ComponentBatch>> as_batches(
            const archetypes::RecordingProperties& archetype
        );
    };
} // namespace rerun
