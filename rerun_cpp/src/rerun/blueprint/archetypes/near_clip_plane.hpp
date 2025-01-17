// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/near_clip_plane.fbs".

#pragma once

#include "../../blueprint/components/near_clip_plane.hpp"
#include "../../collection.hpp"
#include "../../component_batch.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Controls the distance to the near clip plane in 3D scene units.
    struct NearClipPlane {
        /// Controls the distance to the near clip plane in 3D scene units.
        ///
        /// Content closer than this distance will not be visible.
        rerun::blueprint::components::NearClipPlane near_clip_plane;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.NearClipPlaneIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.NearClipPlane";

      public:
        NearClipPlane() = default;
        NearClipPlane(NearClipPlane&& other) = default;
        NearClipPlane(const NearClipPlane& other) = default;
        NearClipPlane& operator=(const NearClipPlane& other) = default;
        NearClipPlane& operator=(NearClipPlane&& other) = default;

        explicit NearClipPlane(rerun::blueprint::components::NearClipPlane _near_clip_plane)
            : near_clip_plane(std::move(_near_clip_plane)) {}
    };

} // namespace rerun::blueprint::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<blueprint::archetypes::NearClipPlane> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::NearClipPlane& archetype
        );
    };
} // namespace rerun
