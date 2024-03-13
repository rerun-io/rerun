// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/archetypes/background_3d.fbs".

#include "background3d.hpp"

#include "../../collection_adapter_builtins.hpp"

namespace rerun::blueprint::archetypes {}

namespace rerun {

    Result<std::vector<DataCell>> AsComponents<blueprint::archetypes::Background3D>::serialize(
        const blueprint::archetypes::Background3D& archetype
    ) {
        using namespace blueprint::archetypes;
        std::vector<DataCell> cells;
        cells.reserve(3);

        {
            auto result = DataCell::from_loggable(archetype.kind);
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.color.has_value()) {
            auto result = DataCell::from_loggable(archetype.color.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = Background3D::IndicatorComponent();
            auto result = DataCell::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
