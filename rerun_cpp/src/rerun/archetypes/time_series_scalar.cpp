// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/time_series_scalar.fbs".

#include "time_series_scalar.hpp"

#include "../collection_adapter_builtins.hpp"

namespace rerun::archetypes {
    const char TimeSeriesScalar::INDICATOR_COMPONENT_NAME[] =
        "rerun.components.TimeSeriesScalarIndicator";
}

namespace rerun {

    Result<std::vector<SerializedComponentBatch>> AsComponents<
        archetypes::TimeSeriesScalar>::serialize(const archetypes::TimeSeriesScalar& archetype) {
        using namespace archetypes;
        std::vector<SerializedComponentBatch> cells;
        cells.reserve(5);

        {
            auto result = Collection<rerun::components::Scalar>(archetype.scalar).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.radius.has_value()) {
            auto result =
                Collection<rerun::components::Radius>(archetype.radius.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.color.has_value()) {
            auto result = Collection<rerun::components::Color>(archetype.color.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.label.has_value()) {
            auto result = Collection<rerun::components::Text>(archetype.label.value()).serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        if (archetype.scattered.has_value()) {
            auto result =
                Collection<rerun::components::ScalarScattering>(archetype.scattered.value())
                    .serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }
        {
            auto result = Collection<TimeSeriesScalar::IndicatorComponent>(
                              TimeSeriesScalar::IndicatorComponent()
            )
                              .serialize();
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
