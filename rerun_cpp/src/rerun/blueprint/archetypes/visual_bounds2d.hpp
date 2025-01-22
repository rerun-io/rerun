// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/visual_bounds2d.fbs".

#pragma once

#include "../../blueprint/components/visual_bounds2d.hpp"
#include "../../collection.hpp"
#include "../../compiler_utils.hpp"
#include "../../component_batch.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
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
        std::optional<ComponentBatch> range;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.VisualBounds2DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.VisualBounds2D";

        /// `ComponentDescriptor` for the `range` field.
        static constexpr auto Descriptor_range = ComponentDescriptor(
            ArchetypeName, "range",
            Loggable<rerun::blueprint::components::VisualBounds2D>::Descriptor.component_name
        );

      public:
        VisualBounds2D() = default;
        VisualBounds2D(VisualBounds2D&& other) = default;
        VisualBounds2D(const VisualBounds2D& other) = default;
        VisualBounds2D& operator=(const VisualBounds2D& other) = default;
        VisualBounds2D& operator=(VisualBounds2D&& other) = default;

        explicit VisualBounds2D(rerun::blueprint::components::VisualBounds2D _range)
            : range(ComponentBatch::from_loggable(std::move(_range), Descriptor_range)
                        .value_or_throw()) {}

        /// Update only some specific fields of a `VisualBounds2D`.
        static VisualBounds2D update_fields() {
            return VisualBounds2D();
        }

        /// Clear all the fields of a `VisualBounds2D`.
        static VisualBounds2D clear_fields();

        /// Controls the visible range of a 2D view.
        ///
        /// Use this to control pan & zoom of the view.
        VisualBounds2D with_range(const rerun::blueprint::components::VisualBounds2D& _range) && {
            range = ComponentBatch::from_loggable(_range, Descriptor_range).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }
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
