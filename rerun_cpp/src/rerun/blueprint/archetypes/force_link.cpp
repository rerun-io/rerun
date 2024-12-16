// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/force_link.fbs".

#include "../../collection_adapter_builtins.hpp"
#include "force_link.hpp"

namespace rerun::blueprint::archetypes {}

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<blueprint::archetypes::ForceLink>::serialize(
        const blueprint::archetypes::ForceLink& archetype
    ) {
        using namespace blueprint::archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(4);

        if (archetype.enabled.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.enabled.value(),
                ComponentDescriptor(
                    "rerun.blueprint.archetypes.ForceLink",
                    "enabled",
                    "rerun.blueprint.components.Enabled"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.distance.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.distance.value(),
                ComponentDescriptor(
                    "rerun.blueprint.archetypes.ForceLink",
                    "distance",
                    "rerun.blueprint.components.ForceDistance"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.iterations.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.iterations.value(),
                ComponentDescriptor(
                    "rerun.blueprint.archetypes.ForceLink",
                    "iterations",
                    "rerun.blueprint.components.ForceIterations"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = ForceLink::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
