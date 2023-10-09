// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/text_document.fbs".

#include "text_document.hpp"

#include "../indicator_component.hpp"

namespace rerun {
    namespace archetypes {
        const char TextDocument::INDICATOR_COMPONENT_NAME[] =
            "rerun.components.TextDocumentIndicator";

        Result<std::vector<SerializedComponentBatch>> TextDocument::serialize() const {
            std::vector<SerializedComponentBatch> cells;
            cells.reserve(2);

            {
                auto result = ComponentBatch(text).serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            if (media_type.has_value()) {
                auto result = ComponentBatch(media_type.value()).serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }
            {
                components::IndicatorComponent<TextDocument::INDICATOR_COMPONENT_NAME> indicator;
                auto result = ComponentBatch(indicator).serialize();
                RR_RETURN_NOT_OK(result.error);
                cells.emplace_back(std::move(result.value));
            }

            return cells;
        }
    } // namespace archetypes
} // namespace rerun
