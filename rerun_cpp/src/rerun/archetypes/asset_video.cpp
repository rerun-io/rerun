// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/asset_video.fbs".

#include "asset_video.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::AssetVideo>::serialize(
        const archetypes::AssetVideo& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(3);

        {
            auto result = ComponentBatch::from_loggable(archetype.blob);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.media_type.has_value()) {
            auto result = ComponentBatch::from_loggable(archetype.media_type.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = AssetVideo::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
