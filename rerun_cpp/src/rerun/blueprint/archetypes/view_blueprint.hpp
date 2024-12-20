// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/blueprint/archetypes/view_blueprint.fbs".

#pragma once

#include "../../blueprint/components/view_class.hpp"
#include "../../blueprint/components/view_origin.hpp"
#include "../../blueprint/components/visible.hpp"
#include "../../collection.hpp"
#include "../../compiler_utils.hpp"
#include "../../component_batch.hpp"
#include "../../components/name.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: The description of a single view.
    struct ViewBlueprint {
        /// The class of the view.
        rerun::blueprint::components::ViewClass class_identifier;

        /// The name of the view.
        std::optional<rerun::components::Name> display_name;

        /// The "anchor point" of this view.
        ///
        /// Defaults to the root path '/' if not specified.
        ///
        /// The transform at this path forms the reference point for all scene->world transforms in this view.
        /// I.e. the position of this entity path in space forms the origin of the coordinate system in this view.
        /// Furthermore, this is the primary indicator for heuristics on what entities we show in this view.
        std::optional<rerun::blueprint::components::ViewOrigin> space_origin;

        /// Whether this view is visible.
        ///
        /// Defaults to true if not specified.
        std::optional<rerun::blueprint::components::Visible> visible;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.ViewBlueprintIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        ViewBlueprint() = default;
        ViewBlueprint(ViewBlueprint&& other) = default;

        explicit ViewBlueprint(rerun::blueprint::components::ViewClass _class_identifier)
            : class_identifier(std::move(_class_identifier)) {}

        /// The name of the view.
        ViewBlueprint with_display_name(rerun::components::Name _display_name) && {
            display_name = std::move(_display_name);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// The "anchor point" of this view.
        ///
        /// Defaults to the root path '/' if not specified.
        ///
        /// The transform at this path forms the reference point for all scene->world transforms in this view.
        /// I.e. the position of this entity path in space forms the origin of the coordinate system in this view.
        /// Furthermore, this is the primary indicator for heuristics on what entities we show in this view.
        ViewBlueprint with_space_origin(rerun::blueprint::components::ViewOrigin _space_origin) && {
            space_origin = std::move(_space_origin);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Whether this view is visible.
        ///
        /// Defaults to true if not specified.
        ViewBlueprint with_visible(rerun::blueprint::components::Visible _visible) && {
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
    struct AsComponents<blueprint::archetypes::ViewBlueprint> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const blueprint::archetypes::ViewBlueprint& archetype
        );
    };
} // namespace rerun
