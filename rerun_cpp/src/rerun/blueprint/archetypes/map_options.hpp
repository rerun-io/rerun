// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/map_options.fbs".

#pragma once

#include "../../blueprint/components/map_provider.hpp"
#include "../../blueprint/components/zoom_level.hpp"
#include "../../collection.hpp"
#include "../../component_batch.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Configuration for the background of a view.
    struct MapOptions {
        /// Map provider and style to use.
        rerun::blueprint::components::MapProvider provider;

        /// Zoom level for the map. The default is 16.
        rerun::blueprint::components::ZoomLevel zoom;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.MapOptionsIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        MapOptions() = default;
        MapOptions(MapOptions&& other) = default;

        explicit MapOptions(
            rerun::blueprint::components::MapProvider _provider,
            rerun::blueprint::components::ZoomLevel _zoom
        )
            : provider(std::move(_provider)), zoom(std::move(_zoom)) {}
    };

} // namespace rerun::blueprint::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<blueprint::archetypes::MapOptions> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::MapOptions& archetype
        );
    };
} // namespace rerun
