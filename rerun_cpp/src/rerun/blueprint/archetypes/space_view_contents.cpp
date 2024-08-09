// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/space_view_contents.fbs".

#include "space_view_contents.hpp"

#include "../../collection_adapter_builtins.hpp"

namespace rerun::blueprint::archetypes {}

namespace rerun {

    Result<std::vector<ComponentBatch>>
        AsComponents<blueprint::archetypes::SpaceViewContents>::serialize(
            const blueprint::archetypes::SpaceViewContents& archetype
        ) {
        using namespace blueprint::archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(2);

        {
            auto result = ComponentBatch::from_loggable(archetype.query);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = SpaceViewContents::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
