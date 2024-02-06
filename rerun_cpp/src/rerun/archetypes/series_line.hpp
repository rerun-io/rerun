// DO NOT EDIT! This file was auto-generated by crates/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/re_types/definitions/rerun/archetypes/series_line.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../components/color.hpp"
#include "../components/name.hpp"
#include "../components/stroke_width.hpp"
#include "../data_cell.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: Define the style properties for a line series in a chart.
    struct SeriesLine {
        /// Color for the corresponding series.
        std::optional<rerun::components::Color> color;

        /// Stroke width for the corresponding series.
        std::optional<rerun::components::StrokeWidth> width;

        /// Display name of the series.
        ///
        /// Used in the legend.
        std::optional<rerun::components::Name> name;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.SeriesLineIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;

      public:
        SeriesLine() = default;
        SeriesLine(SeriesLine&& other) = default;

        /// Color for the corresponding series.
        SeriesLine with_color(rerun::components::Color _color) && {
            color = std::move(_color);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Stroke width for the corresponding series.
        SeriesLine with_width(rerun::components::StrokeWidth _width) && {
            width = std::move(_width);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Display name of the series.
        ///
        /// Used in the legend.
        SeriesLine with_name(rerun::components::Name _name) && {
            name = std::move(_name);
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
    struct AsComponents<archetypes::SeriesLine> {
        /// Serialize all set component batches.
        static Result<std::vector<DataCell>> serialize(const archetypes::SeriesLine& archetype);
    };
} // namespace rerun
