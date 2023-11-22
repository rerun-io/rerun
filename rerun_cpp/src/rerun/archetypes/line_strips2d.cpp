// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/line_strips2d.fbs".

#include "line_strips2d.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<DataCell>> AsComponents<archetypes::LineStrips2D>::serialize(
        const archetypes::LineStrips2D& archetype
    ) {
        using namespace archetypes;
        std::vector<DataCell> cells;
        cells.reserve(8);

        {
            auto result = Loggable<rerun::components::LineStrip2D>::to_arrow(
                archetype.strips.data(),
                archetype.strips.size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.radii.has_value()) {
            auto result = Loggable<rerun::components::Radius>::to_arrow(
                archetype.radii.value().data(),
                archetype.radii.value().size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.colors.has_value()) {
            auto result = Loggable<rerun::components::Color>::to_arrow(
                archetype.colors.value().data(),
                archetype.colors.value().size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.labels.has_value()) {
            auto result = Loggable<rerun::components::Text>::to_arrow(
                archetype.labels.value().data(),
                archetype.labels.value().size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.draw_order.has_value()) {
            auto result =
                Loggable<rerun::components::DrawOrder>::to_arrow(&archetype.draw_order.value(), 1);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.class_ids.has_value()) {
            auto result = Loggable<rerun::components::ClassId>::to_arrow(
                archetype.class_ids.value().data(),
                archetype.class_ids.value().size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.instance_keys.has_value()) {
            auto result = Loggable<rerun::components::InstanceKey>::to_arrow(
                archetype.instance_keys.value().data(),
                archetype.instance_keys.value().size()
            );
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto indicator = LineStrips2D::IndicatorComponent();
            auto result = Loggable<LineStrips2D::IndicatorComponent>::to_arrow(&indicator, 1);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
