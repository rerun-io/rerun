// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/time_series_scalar.fbs".

#include "time_series_scalar.hpp"

#include "../indicator_component.hpp"

namespace rerun {
    namespace archetypes {
        const char TimeSeriesScalar::INDICATOR_COMPONENT_NAME[] =
            "rerun.components.TimeSeriesScalarIndicator";

        AnonymousComponentBatch TimeSeriesScalar::indicator() {
            return ComponentBatch<
                components::IndicatorComponent<TimeSeriesScalar::INDICATOR_COMPONENT_NAME>>(
                nullptr,
                1
            );
        }

        std::vector<AnonymousComponentBatch> TimeSeriesScalar::as_component_batches() const {
            std::vector<AnonymousComponentBatch> comp_batches;
            comp_batches.reserve(5);

            comp_batches.emplace_back(scalar);
            if (radius.has_value()) {
                comp_batches.emplace_back(radius.value());
            }
            if (color.has_value()) {
                comp_batches.emplace_back(color.value());
            }
            if (label.has_value()) {
                comp_batches.emplace_back(label.value());
            }
            if (scattered.has_value()) {
                comp_batches.emplace_back(scattered.value());
            }
            comp_batches.emplace_back(TimeSeriesScalar::indicator());

            return comp_batches;
        }
    } // namespace archetypes
} // namespace rerun
