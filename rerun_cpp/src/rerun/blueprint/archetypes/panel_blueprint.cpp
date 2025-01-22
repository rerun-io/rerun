// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/panel_blueprint.fbs".

#include "panel_blueprint.hpp"

#include "../../collection_adapter_builtins.hpp"

namespace rerun::blueprint::archetypes {
    PanelBlueprint PanelBlueprint::clear_fields() {
        auto archetype = PanelBlueprint();
        archetype.state =
            ComponentBatch::empty<rerun::blueprint::components::PanelState>(Descriptor_state)
                .value_or_throw();
        return archetype;
    }
} // namespace rerun::blueprint::archetypes

namespace rerun {

    Result<std::vector<ComponentBatch>>
        AsComponents<blueprint::archetypes::PanelBlueprint>::serialize(
            const blueprint::archetypes::PanelBlueprint& archetype
        ) {
        using namespace blueprint::archetypes;
        std::vector<ComponentBatch> cells;
        cells.reserve(2);

        if (archetype.state.has_value()) {
            cells.push_back(archetype.state.value());
        }
        {
            auto indicator = PanelBlueprint::IndicatorComponent();
            auto result = ComponentBatch::from_loggable(indicator);
            RR_RETURN_NOT_OK(result.error);
            cells.emplace_back(std::move(result.value));
        }

        return cells;
    }
} // namespace rerun
