// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/tensor_scalar_mapping.fbs".

#include "../../collection_adapter_builtins.hpp"
#include "tensor_scalar_mapping.hpp"

namespace rerun::blueprint::archetypes {}

namespace rerun {

    Result<std::vector<ComponentBatch>>
        AsComponents<blueprint::archetypes::TensorScalarMapping>::serialize(
            const blueprint::archetypes::TensorScalarMapping& archetype
        ) {
        using namespace blueprint::archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(4);

        if (archetype.mag_filter.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.mag_filter.value(),
                ComponentDescriptor(
                    "rerun.blueprint.archetypes.TensorScalarMapping",
                    "mag_filter",
                    "rerun.components.MagnificationFilter"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.colormap.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.colormap.value(),
                ComponentDescriptor(
                    "rerun.blueprint.archetypes.TensorScalarMapping",
                    "colormap",
                    "rerun.components.Colormap"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.gamma.has_value()) {
            auto result = ComponentBatch::from_loggable(
                archetype.gamma.value(),
                ComponentDescriptor(
                    "rerun.blueprint.archetypes.TensorScalarMapping",
                    "gamma",
                    "rerun.components.GammaCorrection"
                )
            );
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = TensorScalarMapping::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
