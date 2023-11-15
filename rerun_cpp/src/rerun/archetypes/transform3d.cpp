// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/transform3d.fbs".

#include "transform3d.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    const char Transform3D::INDICATOR_COMPONENT_NAME[] = "rerun.components.Transform3DIndicator";
}

namespace rerun {

    Result<std::vector<SerializedComponentBatch>> AsComponents<archetypes::Transform3D>::serialize(
        const archetypes::Transform3D& archetype
    ) {
        using namespace archetypes;
        std::vector<SerializedComponentBatch> cells;
        cells.reserve(1);

        {
            auto result =
                Collection<rerun::components::Transform3D>(archetype.transform).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result =
                Collection<Transform3D::IndicatorComponent>(Transform3D::IndicatorComponent())
                    .serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
