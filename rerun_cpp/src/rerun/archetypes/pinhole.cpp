// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/pinhole.fbs".

#include "pinhole.hpp"

#include "../indicator_component.hpp"

namespace rerun {
    namespace archetypes {
        const char Pinhole::INDICATOR_COMPONENT_NAME[] = "rerun.components.PinholeIndicator";

        Result<std::vector<SerializedComponentBatch>> Pinhole::serialize() const {
            std::vector<SerializedComponentBatch> cells;
            cells.reserve(3);

            {
                auto result = ComponentBatch(image_from_camera).serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (resolution.has_value()) {
                auto result = ComponentBatch(resolution.value()).serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (camera_xyz.has_value()) {
                auto result = ComponentBatch(camera_xyz.value()).serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                components::IndicatorComponent<Pinhole::INDICATOR_COMPONENT_NAME> indicator;
                auto result = ComponentBatch(indicator).serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
