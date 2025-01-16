// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/plot_legend.fbs".

#pragma once

#include "../../blueprint/components/corner2d.hpp"
#include "../../blueprint/components/visible.hpp"
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
    /// **Archetype**: Configuration for the legend of a plot.
    struct PlotLegend {
        /// To what corner the legend is aligned.
        ///
        /// Defaults to the right bottom corner.
        std::optional<rerun::blueprint::components::Corner2D> corner;

        /// Whether the legend is shown at all.
        ///
        /// True by default.
        std::optional<rerun::blueprint::components::Visible> visible;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.PlotLegendIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.PlotLegend";

      public:
        PlotLegend() = default;
        PlotLegend(PlotLegend&& other) = default;

        /// To what corner the legend is aligned.
        ///
        /// Defaults to the right bottom corner.
        PlotLegend with_corner(rerun::blueprint::components::Corner2D _corner) && {
            corner = std::move(_corner);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Whether the legend is shown at all.
        ///
        /// True by default.
        PlotLegend with_visible(rerun::blueprint::components::Visible _visible) && {
            visible = std::move(_visible);
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
    struct AsComponents<blueprint::archetypes::PlotLegend> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::PlotLegend& archetype
        );
    };
} // namespace rerun
