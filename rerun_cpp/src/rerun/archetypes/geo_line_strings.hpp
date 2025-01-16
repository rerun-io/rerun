// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/geo_line_strings.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../component_batch.hpp"
#include "../components/color.hpp"
#include "../components/geo_line_string.hpp"
#include "../components/radius.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: Geospatial line strings with positions expressed in [EPSG:4326](https://epsg.io/4326) altitude and longitude (North/East-positive degrees), and optional colors and radii.
    ///
    /// Also known as "line strips" or "polylines".
    ///
    /// ## Example
    ///
    /// ### Log a geospatial line string
    /// ![image](https://static.rerun.io/geo_line_strings_simple/5669983eb10906ace303755b5b5039cad75b917f/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_geo_line_strings");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     auto line_string = rerun::components::GeoLineString::from_lat_lon(
    ///         {{41.0000, -109.0452},
    ///          {41.0000, -102.0415},
    ///          {36.9931, -102.0415},
    ///          {36.9931, -109.0452},
    ///          {41.0000, -109.0452}}
    ///     );
    ///
    ///     rec.log(
    ///         "colorado",
    ///         rerun::GeoLineStrings(line_string)
    ///             .with_radii(rerun::Radius::ui_points(2.0f))
    ///             .with_colors(rerun::Color(0, 0, 255))
    ///     );
    /// }
    /// ```
    struct GeoLineStrings {
        /// The line strings, expressed in [EPSG:4326](https://epsg.io/4326) coordinates (North/East-positive degrees).
        Collection<rerun::components::GeoLineString> line_strings;

        /// Optional radii for the line strings.
        ///
        /// *Note*: scene units radiii are interpreted as meters. Currently, the display scale only considers the latitude of
        /// the first vertex of each line string (see [this issue](https://github.com/rerun-io/rerun/issues/8013)).
        std::optional<Collection<rerun::components::Radius>> radii;

        /// Optional colors for the line strings.
        std::optional<Collection<rerun::components::Color>> colors;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.GeoLineStringsIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        static constexpr const char ArchetypeName[] = "rerun.archetypes.GeoLineStrings";

      public:
        GeoLineStrings() = default;
        GeoLineStrings(GeoLineStrings&& other) = default;

        explicit GeoLineStrings(Collection<rerun::components::GeoLineString> _line_strings)
            : line_strings(std::move(_line_strings)) {}

        /// Optional radii for the line strings.
        ///
        /// *Note*: scene units radiii are interpreted as meters. Currently, the display scale only considers the latitude of
        /// the first vertex of each line string (see [this issue](https://github.com/rerun-io/rerun/issues/8013)).
        GeoLineStrings with_radii(Collection<rerun::components::Radius> _radii) && {
            radii = std::move(_radii);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional colors for the line strings.
        GeoLineStrings with_colors(Collection<rerun::components::Color> _colors) && {
            colors = std::move(_colors);
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
    struct AsComponents<archetypes::GeoLineStrings> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(
            const archetypes::GeoLineStrings& archetype
        );
    };
} // namespace rerun
