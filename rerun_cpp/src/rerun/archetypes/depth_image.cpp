// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/depth_image.fbs".

#include "depth_image.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    const char DepthImage::INDICATOR_COMPONENT_NAME[] = "rerun.components.DepthImageIndicator";
}

namespace rerun {

    Result<std::vector<DataCell>> AsComponents<archetypes::DepthImage>::serialize(
        const archetypes::DepthImage& archetype
    ) {
        using namespace archetypes;
        std::vector<DataCell> cells;
        cells.reserve(4);

        {
            auto result = rerun::components::TensorData::to_data_cell(&archetype.data, 1);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.meter.has_value()) {
            auto result = rerun::components::DepthMeter::to_data_cell(&archetype.meter.value(), 1);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.draw_order.has_value()) {
            auto result =
                rerun::components::DrawOrder::to_data_cell(&archetype.draw_order.value(), 1);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto indicator = DepthImage::IndicatorComponent();
            auto result = DepthImage::IndicatorComponent::to_data_cell(&indicator, 1);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
