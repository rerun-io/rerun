// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/series_line.fbs".

#include "series_line.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::SeriesLine>::serialize(
        const archetypes::SeriesLine& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(5);

        if (archetype.color.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.color.value(),
                ComponentDescriptor(
                    "rerun.archetypes.SeriesLine",
                    "color",
                    "rerun.components.Color"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.width.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.width.value(),
                ComponentDescriptor(
                    "rerun.archetypes.SeriesLine",
                    "width",
                    "rerun.components.StrokeWidth"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.name.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.name.value(),
                ComponentDescriptor("rerun.archetypes.SeriesLine", "name", "rerun.components.Name")
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.aggregation_policy.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.aggregation_policy.value(),
                ComponentDescriptor(
                    "rerun.archetypes.SeriesLine",
                    "aggregation_policy",
                    "rerun.components.AggregationPolicy"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = SeriesLine::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
