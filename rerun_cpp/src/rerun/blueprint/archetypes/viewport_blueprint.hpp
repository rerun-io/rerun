// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/archetypes/viewport_blueprint.fbs".

#pragma once

#include "../../blueprint/components/auto_layout.hpp"
#include "../../blueprint/components/auto_space_views.hpp"
#include "../../blueprint/components/included_space_views.hpp"
#include "../../blueprint/components/space_view_maximized.hpp"
#include "../../blueprint/components/viewport_layout.hpp"
#include "../../collection.hpp"
#include "../../compiler_utils.hpp"
#include "../../data_cell.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: The top-level description of the Viewport.
    struct ViewportBlueprint {
        /// All of the space-views that belong to the viewport.
        rerun::blueprint::components::IncludedSpaceViews space_views;

        /// The layout of the space-views
        rerun::blueprint::components::ViewportLayout layout;

        /// Show one tab as maximized?
        std::optional<rerun::blueprint::components::SpaceViewMaximized> maximized;

        /// Whether the viewport layout is determined automatically.
        ///
        /// Set to `false` the first time the user messes around with the viewport blueprint.
        std::optional<rerun::blueprint::components::AutoLayout> auto_layout;

        /// Whether or not space views should be created automatically.
        std::optional<rerun::blueprint::components::AutoSpaceViews> auto_space_views;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.ViewportBlueprintIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        ViewportBlueprint() = default;
        ViewportBlueprint(ViewportBlueprint&& other) = default;

        explicit ViewportBlueprint(
            rerun::blueprint::components::IncludedSpaceViews _space_views,
            rerun::blueprint::components::ViewportLayout _layout
        )
            : space_views(std::move(_space_views)), layout(std::move(_layout)) {}

        /// Show one tab as maximized?
        ViewportBlueprint with_maximized(rerun::blueprint::components::SpaceViewMaximized _maximized
        ) && {
            maximized = std::move(_maximized);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Whether the viewport layout is determined automatically.
        ///
        /// Set to `false` the first time the user messes around with the viewport blueprint.
        ViewportBlueprint with_auto_layout(rerun::blueprint::components::AutoLayout _auto_layout
        ) && {
            auto_layout = std::move(_auto_layout);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Whether or not space views should be created automatically.
        ViewportBlueprint with_auto_space_views(
            rerun::blueprint::components::AutoSpaceViews _auto_space_views
        ) && {
            auto_space_views = std::move(_auto_space_views);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Returns the number of primary instances of this archetype.
        size_t num_instances() const {
            return 1;
        }
    };

} // namespace rerun::blueprint::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<blueprint::archetypes::ViewportBlueprint> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(
            const blueprint::archetypes::ViewportBlueprint& archetype
        );
    };
} // namespace rerun
