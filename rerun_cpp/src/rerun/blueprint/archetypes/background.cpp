// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/background.fbs".

#include "background.hpp"

#include "../../collection_adapter_builtins.hpp"

namespace rerun::blueprint::archetypes {}

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<blueprint::archetypes::Background>::serialize(
        const blueprint::archetypes::Background& archetype
    ) {
        using namespace blueprint::archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(3);

        {
            auto result = ComponentBatch::from_loggable(
                archetype.kind,
                ComponentDescriptor(
                    "rerun.blueprint.archetypes.Background",
                    "kind",
                    "rerun.blueprint.components.BackgroundKind"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.color.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.color.value(),
                ComponentDescriptor(
                    "rerun.blueprint.archetypes.Background",
                    "color",
                    "rerun.components.Color"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = Background::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
