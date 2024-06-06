// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/axes3d.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../components/axis_length.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: This archetype shows a set of orthogonal coordinate axes such as for representing a transform.
    ///
    /// See `rerun::archetypes::Transform3D`
    struct Axes3D {
        /// Length of the 3 axes.
        std::optional<rerun::components::AxisLength> length;

      public:
        static constexpr const char IndicatorComponentName[] = "rerun.components.Axes3DIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        Axes3D() = default;
        Axes3D(Axes3D&& other) = default;

        /// Length of the 3 axes.
        Axes3D with_length(rerun::components::AxisLength _length) && {
            length = std::move(_length);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::Axes3D> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(const archetypes::Axes3D& archetype);
    };
} // namespace rerun
