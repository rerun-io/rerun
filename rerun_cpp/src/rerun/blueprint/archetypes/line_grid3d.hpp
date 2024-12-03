// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/line_grid_3d.fbs".

#pragma once

#include "../../blueprint/components/grid_spacing.hpp"
#include "../../blueprint/components/ui_radius.hpp"
#include "../../blueprint/components/visible.hpp"
#include "../../collection.hpp"
#include "../../compiler_utils.hpp"
#include "../../component_batch.hpp"
#include "../../components/color.hpp"
#include "../../components/plane3d.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Configuration for the 3D line grid.
    struct LineGrid3D {
        /// Whether the grid is visible.
        ///
        /// Defaults to true.
        std::optional<rerun::blueprint::components::Visible> visible;

        /// Space between grid lines spacing of one line to the next in scene units.
        std::optional<rerun::blueprint::components::GridSpacing> spacing;

        /// In what plane the grid is drawn.
        ///
        /// Defaults to whatever plane is determined as the plane at zero units up/down as defined by [`archetype.ViewCoordinates`] if present.
        std::optional<rerun::components::Plane3D> plane;

        /// How thick the lines should be in ui units.
        ///
        /// Default is 0.5 ui unit.
        std::optional<rerun::blueprint::components::UiRadius> line_radius;

        /// Color used for the grid.
        ///
        /// Transparency via alpha channel is supported.
        /// Defaults to a slightly transparent light gray.
        std::optional<rerun::components::Color> color;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.LineGrid3DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        LineGrid3D() = default;
        LineGrid3D(LineGrid3D&& other) = default;

        /// Whether the grid is visible.
        ///
        /// Defaults to true.
        LineGrid3D with_visible(rerun::blueprint::components::Visible _visible) && {
            visible = std::move(_visible);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Space between grid lines spacing of one line to the next in scene units.
        LineGrid3D with_spacing(rerun::blueprint::components::GridSpacing _spacing) && {
            spacing = std::move(_spacing);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// In what plane the grid is drawn.
        ///
        /// Defaults to whatever plane is determined as the plane at zero units up/down as defined by [`archetype.ViewCoordinates`] if present.
        LineGrid3D with_plane(rerun::components::Plane3D _plane) && {
            plane = std::move(_plane);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// How thick the lines should be in ui units.
        ///
        /// Default is 0.5 ui unit.
        LineGrid3D with_line_radius(rerun::blueprint::components::UiRadius _line_radius) && {
            line_radius = std::move(_line_radius);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Color used for the grid.
        ///
        /// Transparency via alpha channel is supported.
        /// Defaults to a slightly transparent light gray.
        LineGrid3D with_color(rerun::components::Color _color) && {
            color = std::move(_color);
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
    struct AsComponents<blueprint::archetypes::LineGrid3D> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::LineGrid3D& archetype
        );
    };
} // namespace rerun
