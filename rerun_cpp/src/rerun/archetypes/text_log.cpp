// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/text_log.fbs".

#include "text_log.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    const char TextLog::INDICATOR_COMPONENT_NAME[] = "rerun.components.TextLogIndicator";
}

namespace rerun {

    Result<std::vector<SerializedComponentBatch>> AsComponents<archetypes::TextLog>::serialize(
        const archetypes::TextLog& archetype
    ) {
        using namespace archetypes;
        std::vector<SerializedComponentBatch> cells;
        cells.reserve(3);

        {
            auto result = Collection<rerun::components::Text>(archetype.text).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.level.has_value()) {
            auto result =
                Collection<rerun::components::TextLogLevel>(archetype.level.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.color.has_value()) {
            auto result = Collection<rerun::components::Color>(archetype.color.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result =
                Collection<TextLog::IndicatorComponent>(TextLog::IndicatorComponent()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
