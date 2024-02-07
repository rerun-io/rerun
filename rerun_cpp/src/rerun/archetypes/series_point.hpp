// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/series_point.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../components/color.hpp"
#include "../components/marker_shape.hpp"
#include "../components/marker_size.hpp"
#include "../components/name.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: Define the style properties for a point series in a chart.
    ///
    /// This archetype only provides styling information and should be logged as timeless
    /// when possible. The underlying data needs to be logged to the same entity-path using
    /// the `Scalar` archetype.
    ///
    /// See `rerun::archetypes::Scalar`
    struct SeriesPoint {
        /// Color for the corresponding series.
        std::optional<rerun::components::Color> color;

        /// What shape to use to represent the point
        std::optional<rerun::components::MarkerShape> marker;

        /// Display name of the series.
        ///
        /// Used in the legend.
        std::optional<rerun::components::Name> name;

        /// Size of the marker.
        std::optional<rerun::components::MarkerSize> marker_size;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.SeriesPointIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        SeriesPoint() = default;
        SeriesPoint(SeriesPoint&& other) = default;

        /// Color for the corresponding series.
        SeriesPoint with_color(rerun::components::Color _color) && {
            color = std::move(_color);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// What shape to use to represent the point
        SeriesPoint with_marker(rerun::components::MarkerShape _marker) && {
            marker = std::move(_marker);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Display name of the series.
        ///
        /// Used in the legend.
        SeriesPoint with_name(rerun::components::Name _name) && {
            name = std::move(_name);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Size of the marker.
        SeriesPoint with_marker_size(rerun::components::MarkerSize _marker_size) && {
            marker_size = std::move(_marker_size);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Returns the number of primary instances of this archetype.
        size_t num_instances() const {
            return 0;
        }
    };

} // namespace rerun::archetypes

namespace rerun {
    /// \private
    template <typename T>
    struct AsComponents;

    /// \private
    template <>
    struct AsComponents<archetypes::SeriesPoint> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(const archetypes::SeriesPoint& archetype);
    };
} // namespace rerun
