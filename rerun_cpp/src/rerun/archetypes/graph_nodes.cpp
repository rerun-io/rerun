// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/graph_nodes.fbs".

#include "../collection_adapter_builtins.hpp"
#include "graph_nodes.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::GraphNodes>::serialize(
        const archetypes::GraphNodes& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(7);

        {
            auto result = ComponentBatch::from_loggable(
                archetype.node_ids,
                ComponentDescriptor(
                    "rerun.archetypes.GraphNodes",
                    "node_ids",
                    "rerun.components.GraphNode"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.positions.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.positions.value(),
                ComponentDescriptor(
                    "rerun.archetypes.GraphNodes",
                    "positions",
                    "rerun.components.Position2D"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.colors.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.colors.value(),
                ComponentDescriptor(
                    "rerun.archetypes.GraphNodes",
                    "colors",
                    "rerun.components.Color"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.labels.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.labels.value(),
                ComponentDescriptor(
                    "rerun.archetypes.GraphNodes",
                    "labels",
                    "rerun.components.Text"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.show_labels.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.show_labels.value(),
                ComponentDescriptor(
                    "rerun.archetypes.GraphNodes",
                    "show_labels",
                    "rerun.components.ShowLabels"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.radii.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.radii.value(),
                ComponentDescriptor(
                    "rerun.archetypes.GraphNodes",
                    "radii",
                    "rerun.components.Radius"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = GraphNodes::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
