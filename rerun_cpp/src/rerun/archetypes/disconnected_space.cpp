// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/disconnected_space.fbs".

#include "disconnected_space.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {}

namespace rerun {

    Result<std::vector<DataCell>> AsComponents<archetypes::DisconnectedSpace>::serialize(
        const archetypes::DisconnectedSpace& archetype
    ) {
        using namespace archetypes;
        std::vector<DataCell> cells;
        cells.reserve(2);

        {
            auto result = DataCell::from_loggable<rerun::components::DisconnectedSpace>(
                archetype.disconnected_space
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = DisconnectedSpace::IndicatorComponent();
            auto result = DataCell::from_loggable<decltype(indicator)>(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
