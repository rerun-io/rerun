// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/segmentation_image.fbs".

#include "segmentation_image.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<DataCell>> AsComponents<archetypes::SegmentationImage>::serialize(
        const archetypes::SegmentationImage& archetype
    ) {
        using namespace archetypes;
        std::vector<DataCell> cells;
        cells.reserve(6);

        {
            auto result = DataCell::from_loggable(archetype.data);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result = DataCell::from_loggable(archetype.resolution);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto result = DataCell::from_loggable(archetype.data_type);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.opacity.has_value()) {
            auto result = DataCell::from_loggable(archetype.opacity.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.draw_order.has_value()) {
            auto result = DataCell::from_loggable(archetype.draw_order.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = SegmentationImage::IndicatorComponent();
            auto result = DataCell::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
