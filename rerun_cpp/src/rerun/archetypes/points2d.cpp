// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/points2d.fbs".

#include "../collection_adapter_builtins.hpp"
#include "points2d.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::Points2D>::serialize(
        const archetypes::Points2D& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(9);

        {
            auto result = ComponentBatch::from_loggable(
                archetype.positions,
                ComponentDescriptor(
                    "rerun.archetypes.Points2D",
                    "positions",
                    "rerun.components.Position2D"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.radii.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.radii.value(),
                ComponentDescriptor("rerun.archetypes.Points2D", "radii", "rerun.components.Radius")
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.colors.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.colors.value(),
                ComponentDescriptor("rerun.archetypes.Points2D", "colors", "rerun.components.Color")
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.labels.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.labels.value(),
                ComponentDescriptor("rerun.archetypes.Points2D", "labels", "rerun.components.Text")
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.show_labels.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.show_labels.value(),
                ComponentDescriptor(
                    "rerun.archetypes.Points2D",
                    "show_labels",
                    "rerun.components.ShowLabels"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.draw_order.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.draw_order.value(),
                ComponentDescriptor(
                    "rerun.archetypes.Points2D",
                    "draw_order",
                    "rerun.components.DrawOrder"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.class_ids.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.class_ids.value(),
                ComponentDescriptor(
                    "rerun.archetypes.Points2D",
                    "class_ids",
                    "rerun.components.ClassId"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.keypoint_ids.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.keypoint_ids.value(),
                ComponentDescriptor(
                    "rerun.archetypes.Points2D",
                    "keypoint_ids",
                    "rerun.components.KeypointId"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = Points2D::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
