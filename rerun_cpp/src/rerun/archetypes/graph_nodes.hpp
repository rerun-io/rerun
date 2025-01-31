// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/graph_nodes.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
#include "../components/color.hpp"
#include "../components/graph_node.hpp"
#include "../components/position2d.hpp"
#include "../components/radius.hpp"
#include "../components/show_labels.hpp"
#include "../components/text.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: A list of nodes in a graph with optional labels, colors, etc.
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
    struct GraphNodes {
        /// A list of node IDs.
        std::optional<ComponentBatch> node_ids;

        /// Optional center positions of the nodes.
        std::optional<ComponentBatch> positions;

        /// Optional colors for the boxes.
        std::optional<ComponentBatch> colors;

        /// Optional text labels for the node.
        std::optional<ComponentBatch> labels;

        /// Optional choice of whether the text labels should be shown by default.
        std::optional<ComponentBatch> show_labels;

        /// Optional radii for nodes.
        std::optional<ComponentBatch> radii;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.GraphNodesIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.GraphNodes";

        /// `ComponentDescriptor` for the `node_ids` field.
        static constexpr auto Descriptor_node_ids = ComponentDescriptor(
            ArchetypeName, "node_ids",
            Loggable<rerun::components::GraphNode>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `positions` field.
        static constexpr auto Descriptor_positions = ComponentDescriptor(
            ArchetypeName, "positions",
            Loggable<rerun::components::Position2D>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `colors` field.
        static constexpr auto Descriptor_colors = ComponentDescriptor(
            ArchetypeName, "colors", Loggable<rerun::components::Color>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `labels` field.
        static constexpr auto Descriptor_labels = ComponentDescriptor(
            ArchetypeName, "labels", Loggable<rerun::components::Text>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `show_labels` field.
        static constexpr auto Descriptor_show_labels = ComponentDescriptor(
            ArchetypeName, "show_labels",
            Loggable<rerun::components::ShowLabels>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `radii` field.
        static constexpr auto Descriptor_radii = ComponentDescriptor(
            ArchetypeName, "radii", Loggable<rerun::components::Radius>::Descriptor.component_name
        );

      public:
        GraphNodes() = default;
        GraphNodes(GraphNodes&& other) = default;
        GraphNodes(const GraphNodes& other) = default;
        GraphNodes& operator=(const GraphNodes& other) = default;
        GraphNodes& operator=(GraphNodes&& other) = default;

        explicit GraphNodes(Collection<rerun::components::GraphNode> _node_ids)
            : node_ids(ComponentBatch::from_loggable(std::move(_node_ids), Descriptor_node_ids)
                           .value_or_throw()) {}

        /// Update only some specific fields of a `GraphNodes`.
        static GraphNodes update_fields() {
            return GraphNodes();
        }

        /// Clear all the fields of a `GraphNodes`.
        static GraphNodes clear_fields();

        /// A list of node IDs.
        GraphNodes with_node_ids(const Collection<rerun::components::GraphNode>& _node_ids) && {
            node_ids =
                ComponentBatch::from_loggable(_node_ids, Descriptor_node_ids).value_or_throw();
            return std::move(*this);
        }

        /// Optional center positions of the nodes.
        GraphNodes with_positions(const Collection<rerun::components::Position2D>& _positions) && {
            positions =
                ComponentBatch::from_loggable(_positions, Descriptor_positions).value_or_throw();
            return std::move(*this);
        }

        /// Optional colors for the boxes.
        GraphNodes with_colors(const Collection<rerun::components::Color>& _colors) && {
            colors = ComponentBatch::from_loggable(_colors, Descriptor_colors).value_or_throw();
            return std::move(*this);
        }

        /// Optional text labels for the node.
        GraphNodes with_labels(const Collection<rerun::components::Text>& _labels) && {
            labels = ComponentBatch::from_loggable(_labels, Descriptor_labels).value_or_throw();
            return std::move(*this);
        }

        /// Optional choice of whether the text labels should be shown by default.
        GraphNodes with_show_labels(const rerun::components::ShowLabels& _show_labels) && {
            show_labels = ComponentBatch::from_loggable(_show_labels, Descriptor_show_labels)
                              .value_or_throw();
            return std::move(*this);
        }

        /// This method makes it possible to pack multiple `show_labels` in a single component batch.
        ///
        /// This only makes sense when used in conjunction with `columns`. `with_show_labels` should
        /// be used when logging a single row's worth of data.
        GraphNodes with_many_show_labels(
            const Collection<rerun::components::ShowLabels>& _show_labels
        ) && {
            show_labels = ComponentBatch::from_loggable(_show_labels, Descriptor_show_labels)
                              .value_or_throw();
            return std::move(*this);
        }

        /// Optional radii for nodes.
        GraphNodes with_radii(const Collection<rerun::components::Radius>& _radii) && {
            radii = ComponentBatch::from_loggable(_radii, Descriptor_radii).value_or_throw();
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
    struct AsComponents<archetypes::GraphNodes> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(const archetypes::GraphNodes& archetype
        );
    };
} // namespace rerun
