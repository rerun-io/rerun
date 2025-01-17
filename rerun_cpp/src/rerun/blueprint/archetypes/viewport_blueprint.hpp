// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/viewport_blueprint.fbs".

#pragma once

#include "../../blueprint/components/auto_layout.hpp"
#include "../../blueprint/components/auto_views.hpp"
#include "../../blueprint/components/root_container.hpp"
#include "../../blueprint/components/view_maximized.hpp"
#include "../../blueprint/components/viewer_recommendation_hash.hpp"
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
    /// **Archetype**: The top-level description of the viewport.
    struct ViewportBlueprint {
        /// The layout of the views
        std::optional<rerun::blueprint::components::RootContainer> root_container;

        /// Show one tab as maximized?
        std::optional<rerun::blueprint::components::ViewMaximized> maximized;

        /// Whether the viewport layout is determined automatically.
        ///
        /// If `true`, the container layout will be reset whenever a new view is added or removed.
        /// This defaults to `false` and is automatically set to `false` when there is user determined layout.
        std::optional<rerun::blueprint::components::AutoLayout> auto_layout;

        /// Whether or not views should be created automatically.
        ///
        /// If `true`, the viewer will only add views that it hasn't considered previously (as identified by `past_viewer_recommendations`)
        /// and which aren't deemed redundant to existing views.
        /// This defaults to `false` and is automatically set to `false` when the user adds views manually in the viewer.
        std::optional<rerun::blueprint::components::AutoViews> auto_views;

        /// Hashes of all recommended views the viewer has already added and that should not be added again.
        ///
        /// This is an internal field and should not be set usually.
        /// If you want the viewer from stopping to add views, you should set `auto_views` to `false`.
        ///
        /// The viewer uses this to determine whether it should keep adding views.
        std::optional<Collection<rerun::blueprint::components::ViewerRecommendationHash>>
            past_viewer_recommendations;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.ViewportBlueprintIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] =
            "rerun.blueprint.archetypes.ViewportBlueprint";

      public:
        ViewportBlueprint() = default;
        ViewportBlueprint(ViewportBlueprint&& other) = default;
        ViewportBlueprint(const ViewportBlueprint& other) = default;
        ViewportBlueprint& operator=(const ViewportBlueprint& other) = default;
        ViewportBlueprint& operator=(ViewportBlueprint&& other) = default;

        /// The layout of the views
        ViewportBlueprint with_root_container(
            rerun::blueprint::components::RootContainer _root_container
        ) && {
            root_container = std::move(_root_container);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Show one tab as maximized?
        ViewportBlueprint with_maximized(rerun::blueprint::components::ViewMaximized _maximized
        ) && {
            maximized = std::move(_maximized);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Whether the viewport layout is determined automatically.
        ///
        /// If `true`, the container layout will be reset whenever a new view is added or removed.
        /// This defaults to `false` and is automatically set to `false` when there is user determined layout.
        ViewportBlueprint with_auto_layout(rerun::blueprint::components::AutoLayout _auto_layout
        ) && {
            auto_layout = std::move(_auto_layout);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Whether or not views should be created automatically.
        ///
        /// If `true`, the viewer will only add views that it hasn't considered previously (as identified by `past_viewer_recommendations`)
        /// and which aren't deemed redundant to existing views.
        /// This defaults to `false` and is automatically set to `false` when the user adds views manually in the viewer.
        ViewportBlueprint with_auto_views(rerun::blueprint::components::AutoViews _auto_views) && {
            auto_views = std::move(_auto_views);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Hashes of all recommended views the viewer has already added and that should not be added again.
        ///
        /// This is an internal field and should not be set usually.
        /// If you want the viewer from stopping to add views, you should set `auto_views` to `false`.
        ///
        /// The viewer uses this to determine whether it should keep adding views.
        ViewportBlueprint with_past_viewer_recommendations(
            Collection<rerun::blueprint::components::ViewerRecommendationHash>
                _past_viewer_recommendations
        ) && {
            past_viewer_recommendations = std::move(_past_viewer_recommendations);
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
    struct AsComponents<blueprint::archetypes::ViewportBlueprint> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::ViewportBlueprint& archetype
        );
    };
} // namespace rerun
