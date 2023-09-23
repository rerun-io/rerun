// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/bar_chart.fbs".

#include "bar_chart.hpp"

#include "../indicator_component.hpp"

namespace rerun {
    namespace archetypes {
        const char BarChart::INDICATOR_COMPONENT_NAME[] = "rerun.components.BarChartIndicator";

        std::vector<AnonymousComponentBatch> BarChart::as_component_batches() const {
            std::vector<AnonymousComponentBatch> comp_batches;
            comp_batches.reserve(1);

            comp_batches.emplace_back(values);
            comp_batches.emplace_back(
                ComponentBatch<components::IndicatorComponent<BarChart::INDICATOR_COMPONENT_NAME>>(
                    nullptr,
                    num_instances()
                )
            );

            return comp_batches;
        }
    } // namespace archetypes
} // namespace rerun
