// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/arrows2d.fbs".

#include "arrows2d.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<DataCell>> AsComponents<archetypes::Arrows2D>::serialize(
        const archetypes::Arrows2D& archetype
    ) {
        using namespace archetypes;
        std::vector<DataCell> cells;
        cells.reserve(8);

        {
            auto result = DataCell::from_loggable(archetype.vectors);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.origins.has_value()) {
            auto result = DataCell::from_loggable(archetype.origins.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.radii.has_value()) {
            auto result = DataCell::from_loggable(archetype.radii.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.colors.has_value()) {
            auto result = DataCell::from_loggable(archetype.colors.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.labels.has_value()) {
            auto result = DataCell::from_loggable(archetype.labels.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.draw_order.has_value()) {
            auto result = DataCell::from_loggable(archetype.draw_order.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.class_ids.has_value()) {
            auto result = DataCell::from_loggable(archetype.class_ids.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = Arrows2D::IndicatorComponent();
            auto result = DataCell::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
