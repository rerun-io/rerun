// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/text_document.fbs".

#include "text_document.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<DataCell>> AsComponents<archetypes::TextDocument>::serialize(
        const archetypes::TextDocument& archetype
    ) {
        using namespace archetypes;
        std::vector<DataCell> cells;
        cells.reserve(3);

        {
            auto result = DataCell::from_loggable(archetype.text);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.media_type.has_value()) {
            auto result = DataCell::from_loggable(archetype.media_type.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = TextDocument::IndicatorComponent();
            auto result = DataCell::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
