// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/visual_bounds2d.fbs".

#pragma once

#include "../../blueprint/components/visual_bounds2d.hpp"
#include "../../collection.hpp"
#include "../../component_batch.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Controls the visual bounds of a 2D view.
    ///
    /// Everything within these bounds are guaranteed to be visible.
    /// Somethings outside of these bounds may also be visible due to letterboxing.
    ///
    /// If no visual bounds are set, it will be determined automatically,
    /// based on the bounding-box of the data or other camera information present in the view.
    struct VisualBounds2D {
        /// Controls the visible range of a 2D view.
        ///
        /// Use this to control pan & zoom of the view.
        rerun::blueprint::components::VisualBounds2D range;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.VisualBounds2DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.VisualBounds2D";

      public:
        VisualBounds2D() = default;
        VisualBounds2D(VisualBounds2D&& other) = default;

        explicit VisualBounds2D(rerun::blueprint::components::VisualBounds2D _range)
            : range(std::move(_range)) {}
    };

} // namespace rerun::blueprint::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<blueprint::archetypes::VisualBounds2D> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::VisualBounds2D& archetype
        );
    };
} // namespace rerun
