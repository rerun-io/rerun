// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/boxes2d.fbs".

#include "boxes2d.hpp"

#include "../component_batch_adapter_builtins.hpp"

namespace rerun::archetypes {
    const char Boxes2D::INDICATOR_COMPONENT_NAME[] = "rerun.components.Boxes2DIndicator";
}

namespace rerun {

    Result<std::vector<SerializedComponentBatch>> AsComponents<archetypes::Boxes2D>::serialize(
        const archetypes::Boxes2D& archetype
    ) {
        using namespace archetypes;
        std::vector<SerializedComponentBatch> cells;
        cells.reserve(8);

        {
            auto result = (archetype.half_sizes).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.centers.has_value()) {
            auto result = (archetype.centers.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.colors.has_value()) {
            auto result = (archetype.colors.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.radii.has_value()) {
            auto result = (archetype.radii.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.labels.has_value()) {
            auto result = (archetype.labels.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.draw_order.has_value()) {
            auto result = ComponentBatch<rerun::components::DrawOrder>(archetype.draw_order.value())
                              .serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.class_ids.has_value()) {
            auto result = (archetype.class_ids.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.instance_keys.has_value()) {
            auto result = (archetype.instance_keys.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = ComponentBatch<Boxes2D::IndicatorComponent>(Boxes2D::IndicatorComponent())
                              .serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
