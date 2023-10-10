// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/text_log.fbs".

#include "text_log.hpp"

namespace rerun {
    namespace archetypes {
        const char TextLog::INDICATOR_COMPONENT_NAME[] = "rerun.components.TextLogIndicator";

        Result<std::vector<SerializedComponentBatch>> TextLog::serialize() const {
            std::vector<SerializedComponentBatch> cells;
            cells.reserve(3);

            {
                auto result = ComponentBatch<rerun::components::Text>(text).serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (level.has_value()) {
                auto result =
                    ComponentBatch<rerun::components::TextLogLevel>(level.value()).serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (color.has_value()) {
                auto result = ComponentBatch<rerun::components::Color>(color.value()).serialize();
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
