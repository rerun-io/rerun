// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/tensor_slice_selection.fbs".

#include "tensor_slice_selection.hpp"

#include "../../collection_adapter_builtins.hpp"

namespace rerun::blueprint::archetypes {}

namespace rerun {

    Result<std::vector<ComponentBatch>>
        AsComponents<blueprint::archetypes::TensorSliceSelection>::serialize(
            const blueprint::archetypes::TensorSliceSelection& archetype
        ) {
        using namespace blueprint::archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(5);

        if (archetype.width.has_value()) {
            auto result = ComponentBatch::from_loggable(archetype.width.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.height.has_value()) {
            auto result = ComponentBatch::from_loggable(archetype.height.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.indices.has_value()) {
            auto result = ComponentBatch::from_loggable(archetype.indices.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        if (archetype.slider.has_value()) {
            auto result = ComponentBatch::from_loggable(archetype.slider.value());
            RR_RETURN_NOT_OK(result.error);
            cells.push_back(std::move(result.value));
        }
        {
            auto indicator = TensorSliceSelection::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
