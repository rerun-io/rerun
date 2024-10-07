// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/graph_edges.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../component_batch.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/graph_edge_undirected.hpp"
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
    struct GraphEdges {
        /// A list of node IDs.
        Collection<rerun::components::GraphEdgeUndirected> edges;

        /// Optional colors for the boxes.
        std::optional<Collection<rerun::components::Color>> colors;

        /// Optional text labels for the node.
        std::optional<Collection<rerun::components::Text>> labels;

        /// Optional choice of whether the text labels should be shown by default.
        std::optional<rerun::components::ShowLabels> show_labels;

        /// Optional `components::ClassId`s for the boxes.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
        std::optional<Collection<rerun::components::ClassId>> class_ids;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.GraphEdgesIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        GraphEdges() = default;
        GraphEdges(GraphEdges&& other) = default;

        explicit GraphEdges(Collection<rerun::components::GraphEdgeUndirected> _edges)
            : edges(std::move(_edges)) {}

        /// Optional colors for the boxes.
        GraphEdges with_colors(Collection<rerun::components::Color> _colors) && {
            colors = std::move(_colors);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional text labels for the node.
        GraphEdges with_labels(Collection<rerun::components::Text> _labels) && {
            labels = std::move(_labels);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional choice of whether the text labels should be shown by default.
        GraphEdges with_show_labels(rerun::components::ShowLabels _show_labels) && {
            show_labels = std::move(_show_labels);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional `components::ClassId`s for the boxes.
        ///
        /// The `components::ClassId` provides colors and labels if not specified explicitly.
        GraphEdges with_class_ids(Collection<rerun::components::ClassId> _class_ids) && {
            class_ids = std::move(_class_ids);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::GraphEdges> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(const archetypes::GraphEdges& archetype
        );
    };
} // namespace rerun
