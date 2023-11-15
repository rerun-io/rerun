// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/disconnected_space.fbs".

#include "disconnected_space.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    const char DisconnectedSpace::INDICATOR_COMPONENT_NAME[] =
        "rerun.components.DisconnectedSpaceIndicator";
}

namespace rerun {

    Result<std::vector<SerializedComponentBatch>> AsComponents<
        archetypes::DisconnectedSpace>::serialize(const archetypes::DisconnectedSpace& archetype) {
        using namespace archetypes;
        std::vector<SerializedComponentBatch> cells;
        cells.reserve(1);

        {
            auto result =
                Collection<rerun::components::DisconnectedSpace>(archetype.disconnected_space)
                    .serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = Collection<DisconnectedSpace::IndicatorComponent>(
                              DisconnectedSpace::IndicatorComponent()
            )
                              .serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
