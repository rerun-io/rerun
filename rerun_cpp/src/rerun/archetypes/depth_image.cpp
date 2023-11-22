// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/depth_image.fbs".

#include "depth_image.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<DataCell>> AsComponents<archetypes::DepthImage>::serialize(
        const archetypes::DepthImage& archetype
    ) {
        using namespace archetypes;
        std::vector<DataCell> cells;
        cells.reserve(4);

        {
            auto result = DataCell::from_loggable<rerun::components::TensorData>(archetype.data);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.meter.has_value()) {
            auto result =
                DataCell::from_loggable<rerun::components::DepthMeter>(archetype.meter.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.draw_order.has_value()) {
            auto result =
                DataCell::from_loggable<rerun::components::DrawOrder>(archetype.draw_order.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = DepthImage::IndicatorComponent();
            auto result = Loggable<DepthImage::IndicatorComponent>::to_arrow(&indicator, 1);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
