// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/blueprint/archetypes/background_3d.fbs".

#pragma once

#include "../../blueprint/components/background3d_kind.hpp"
#include "../../collection.hpp"
#include "../../compiler_utils.hpp"
#include "../../components/color.hpp"
#include "../../data_cell.hpp"
#include "../../indicator_component.hpp"
#include "../../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::blueprint::archetypes {
    /// **Archetype**: Configuration for the background of the 3D space view.
    struct Background3D {
        /// The type of the background. Defaults to DirectionalGradient
        rerun::blueprint::components::Background3DKind kind;

        /// Color used for Background3DKind.SolidColor.
        ///
        /// Defaults to White.
        std::optional<rerun::components::Color> color;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.blueprint.components.Background3DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        Background3D() = default;
        Background3D(Background3D&& other) = default;

        explicit Background3D(rerun::blueprint::components::Background3DKind _kind)
            : kind(std::move(_kind)) {}

        /// Color used for Background3DKind.SolidColor.
        ///
        /// Defaults to White.
        Background3D with_color(rerun::components::Color _color) && {
            color = std::move(_color);
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
    struct AsComponents<blueprint::archetypes::Background3D> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(
            const blueprint::archetypes::Background3D& archetype
        );
    };
} // namespace rerun
