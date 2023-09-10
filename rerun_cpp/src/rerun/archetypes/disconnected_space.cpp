// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/disconnected_space.fbs".

#include "disconnected_space.hpp"

#include "../indicator_component.hpp"

namespace rerun {
    namespace archetypes {
        const char DisconnectedSpace::INDICATOR_COMPONENT_NAME[] =
            "rerun.components.DisconnectedSpaceIndicator";

        std::vector<AnonymousComponentBatch> DisconnectedSpace::as_component_batches() const {
            std::vector<AnonymousComponentBatch> comp_batches;
            comp_batches.reserve(1);

            comp_batches.emplace_back(disconnected_space);
            comp_batches.emplace_back(
                ComponentBatch<
                    components::IndicatorComponent<DisconnectedSpace::INDICATOR_COMPONENT_NAME>>(
                    nullptr,
                    num_instances()
                )
            );

            return comp_batches;
        }
    } // namespace archetypes
} // namespace rerun
