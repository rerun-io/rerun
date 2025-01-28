// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/geo_line_strings.fbs".

#pragma once

#include "../collection.hpp"
#include "../component_batch.hpp"
#include "../component_column.hpp"
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
        std::optional<ComponentBatch> line_strings;

        /// Optional radii for the line strings.
        ///
        /// *Note*: scene units radiii are interpreted as meters. Currently, the display scale only considers the latitude of
        /// the first vertex of each line string (see [this issue](https://github.com/rerun-io/rerun/issues/8013)).
        std::optional<ComponentBatch> radii;

        /// Optional colors for the line strings.
        std::optional<ComponentBatch> colors;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.GeoLineStringsIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        /// The name of the archetype as used in `ComponentDescriptor`s.
        static constexpr const char ArchetypeName[] = "rerun.archetypes.GeoLineStrings";

        /// `ComponentDescriptor` for the `line_strings` field.
        static constexpr auto Descriptor_line_strings = ComponentDescriptor(
            ArchetypeName, "line_strings",
            Loggable<rerun::components::GeoLineString>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `radii` field.
        static constexpr auto Descriptor_radii = ComponentDescriptor(
            ArchetypeName, "radii", Loggable<rerun::components::Radius>::Descriptor.component_name
        );
        /// `ComponentDescriptor` for the `colors` field.
        static constexpr auto Descriptor_colors = ComponentDescriptor(
            ArchetypeName, "colors", Loggable<rerun::components::Color>::Descriptor.component_name
        );

      public:
        GeoLineStrings() = default;
        GeoLineStrings(GeoLineStrings&& other) = default;
        GeoLineStrings(const GeoLineStrings& other) = default;
        GeoLineStrings& operator=(const GeoLineStrings& other) = default;
        GeoLineStrings& operator=(GeoLineStrings&& other) = default;

        explicit GeoLineStrings(Collection<rerun::components::GeoLineString> _line_strings)
            : line_strings(
                  ComponentBatch::from_loggable(std::move(_line_strings), Descriptor_line_strings)
                      .value_or_throw()
              ) {}

        /// Update only some specific fields of a `GeoLineStrings`.
        static GeoLineStrings update_fields() {
            return GeoLineStrings();
        }

        /// Clear all the fields of a `GeoLineStrings`.
        static GeoLineStrings clear_fields();

        /// The line strings, expressed in [EPSG:4326](https://epsg.io/4326) coordinates (North/East-positive degrees).
        GeoLineStrings with_line_strings(
            const Collection<rerun::components::GeoLineString>& _line_strings
        ) && {
            line_strings = ComponentBatch::from_loggable(_line_strings, Descriptor_line_strings)
                               .value_or_throw();
            return std::move(*this);
        }

        /// Optional radii for the line strings.
        ///
        /// *Note*: scene units radiii are interpreted as meters. Currently, the display scale only considers the latitude of
        /// the first vertex of each line string (see [this issue](https://github.com/rerun-io/rerun/issues/8013)).
        GeoLineStrings with_radii(const Collection<rerun::components::Radius>& _radii) && {
            radii = ComponentBatch::from_loggable(_radii, Descriptor_radii).value_or_throw();
            return std::move(*this);
        }

        /// Optional colors for the line strings.
        GeoLineStrings with_colors(const Collection<rerun::components::Color>& _colors) && {
            colors = ComponentBatch::from_loggable(_colors, Descriptor_colors).value_or_throw();
            return std::move(*this);
        }

        /// Partitions the component data into multiple sub-batches.
        ///
        /// Specifically, this transforms the existing `ComponentBatch` data into `ComponentColumn`s
        /// instead, via `ComponentColumn::from_batch_with_lengths`.
        ///
        /// This makes it possible to use `RecordingStream::send_columns` to send columnar data directly into Rerun.
        ///
        /// The specified `lengths` must sum to the total length of the component batch.
        Collection<ComponentColumn> columns(const Collection<uint32_t>& lengths_);

        /// Partitions the component data into unit-length sub-batches.
        ///
        /// This is semantically similar to calling `columns` with `std::vector<uint32_t>(n, 1)`,
        /// where `n` is automatically guessed.
        Collection<ComponentColumn> columns();
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
