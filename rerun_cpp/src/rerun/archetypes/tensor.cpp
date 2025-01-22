// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/tensor.fbs".

#include "tensor.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    Tensor Tensor::clear_fields() {
        auto archetype = Tensor();
        archetype.data =
            ComponentBatch::empty<rerun::components::TensorData>(Descriptor_data).value_or_throw();
        archetype.value_range =
            ComponentBatch::empty<rerun::components::ValueRange>(Descriptor_value_range)
                .value_or_throw();
        return archetype;
    }
} // namespace rerun::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>> AsComponents<archetypes::Tensor>::serialize(
        const archetypes::Tensor& archetype
    ) {
        using namespace archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(3);

        if (archetype.data.has_value()) {
            cells.push_back(archetype.data.value());
        }
        if (archetype.value_range.has_value()) {
            cells.push_back(archetype.value_range.value());
        }
        {
            auto indicator = Tensor::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
