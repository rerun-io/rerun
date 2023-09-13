// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/clear.fbs".

#include "clear.hpp"

#include "../indicator_component.hpp"

namespace rerun {
    namespace archetypes {
        const char Clear::INDICATOR_COMPONENT_NAME[] = "rerun.components.ClearIndicator";

        std::vector<AnonymousComponentBatch> Clear::as_component_batches() const {
            std::vector<AnonymousComponentBatch> comp_batches;
            comp_batches.reserve(1);

            comp_batches.emplace_back(settings);
            comp_batches.emplace_back(
                ComponentBatch<components::IndicatorComponent<Clear::INDICATOR_COMPONENT_NAME>>(
                    nullptr,
                    num_instances()
                )
            );

            return comp_batches;
        }
    } // namespace archetypes
} // namespace rerun
