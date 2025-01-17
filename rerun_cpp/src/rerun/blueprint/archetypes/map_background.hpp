// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/map_background.fbs".

#pragma once

#include "../../blueprint/components/map_provider.hpp"
#include "../../collection.hpp"
#include "../../component_batch.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Configuration for the background map of the map view.
    struct MapBackground {
        /// Map provider and style to use.
        ///
        /// **Note**: Requires a Mapbox API key in the `RERUN_MAPBOX_ACCESS_TOKEN` environment variable.
        rerun::blueprint::components::MapProvider provider;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.MapBackgroundIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.MapBackground";

      public:
        MapBackground() = default;
        MapBackground(MapBackground&& other) = default;
        MapBackground(const MapBackground& other) = default;
        MapBackground& operator=(const MapBackground& other) = default;
        MapBackground& operator=(MapBackground&& other) = default;

        explicit MapBackground(rerun::blueprint::components::MapProvider _provider)
            : provider(std::move(_provider)) {}
    };

} // namespace rerun::blueprint::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<blueprint::archetypes::MapBackground> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::MapBackground& archetype
        );
    };
} // namespace rerun
