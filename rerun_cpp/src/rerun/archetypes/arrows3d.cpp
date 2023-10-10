// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/arrows3d.fbs".

#include "arrows3d.hpp"

namespace rerun {
    namespace archetypes {
        const char Arrows3D::INDICATOR_COMPONENT_NAME[] = "rerun.components.Arrows3DIndicator";

        Result<std::vector<SerializedComponentBatch>> Arrows3D::serialize() const {
            std::vector<SerializedComponentBatch> cells;
            cells.reserve(7);

            {
                auto result = vectors.serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (origins.has_value()) {
                auto result = origins.value().serialize();
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
            if (class_ids.has_value()) {
                auto result = class_ids.value().serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (instance_keys.has_value()) {
                auto result = instance_keys.value().serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                auto result = ComponentBatch<IndicatorComponent>(IndicatorComponent()).serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
