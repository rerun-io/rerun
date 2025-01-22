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
        std::optional<ComponentBatch> corner;

        /// Whether the legend is shown at all.
        ///
        /// True by default.
        std::optional<ComponentBatch> visible;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.PlotLegendIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.blueprint.archetypes.PlotLegend";

        /// `ComponentDescriptor` for the `corner` field.
        static constexpr auto Descriptor_corner = ComponentDescriptor(
            ArchetypeName, "corner",
            Loggable<rerun::blueprint::components::Corner2D>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `visible` field.
        static constexpr auto Descriptor_visible = ComponentDescriptor(
            ArchetypeName, "visible",
            Loggable<rerun::blueprint::components::Visible>::Descriptor.component_name
        );

      public:
        PlotLegend() = default;
        PlotLegend(PlotLegend&& other) = default;
        PlotLegend(const PlotLegend& other) = default;
        PlotLegend& operator=(const PlotLegend& other) = default;
        PlotLegend& operator=(PlotLegend&& other) = default;

        /// Update only some specific fields of a `PlotLegend`.
        static PlotLegend update_fields() {
            return PlotLegend();
        }

        /// Clear all the fields of a `PlotLegend`.
        static PlotLegend clear_fields();

        /// To what corner the legend is aligned.
        ///
        /// Defaults to the right bottom corner.
        PlotLegend with_corner(const rerun::blueprint::components::Corner2D& _corner) && {
            corner = ComponentBatch::from_loggable(_corner, Descriptor_corner).value_or_throw();
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Whether the legend is shown at all.
        ///
        /// True by default.
        PlotLegend with_visible(const rerun::blueprint::components::Visible& _visible) && {
            visible = ComponentBatch::from_loggable(_visible, Descriptor_visible).value_or_throw();
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
