// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/points2d.fbs".

#include "points2d.hpp"

#include "../indicator_component.hpp"

namespace rerun {
    namespace archetypes {
        const char Points2D::INDICATOR_COMPONENT_NAME[] = "rerun.components.Points2DIndicator";

        Result<std::vector<SerializedComponentBatch>> Points2D::serialize() const {
            std::vector<SerializedComponentBatch> cells;
            cells.reserve(8);

            {
                auto result = positions.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (radii.has_value()) {
                auto result = radii.value().serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (colors.has_value()) {
                auto result = colors.value().serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (labels.has_value()) {
                auto result = labels.value().serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (draw_order.has_value()) {
                auto result = ComponentBatch(draw_order.value()).serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (class_ids.has_value()) {
                auto result = class_ids.value().serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (keypoint_ids.has_value()) {
                auto result = keypoint_ids.value().serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (instance_keys.has_value()) {
                auto result = instance_keys.value().serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                components::IndicatorComponent<Points2D::INDICATOR_COMPONENT_NAME> indicator;
                auto result = ComponentBatch(indicator).serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
