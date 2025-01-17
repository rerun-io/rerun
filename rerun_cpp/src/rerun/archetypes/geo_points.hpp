// DO NOT EDIT! This file was auto-generated by crates/build/re_types_builder/src/codegen/cpp/mod.rs
// Based on "crates/store/re_types/definitions/rerun/archetypes/geo_points.fbs".

#pragma once

#include "../collection.hpp"
#include "../compiler_utils.hpp"
#include "../component_batch.hpp"
#include "../components/class_id.hpp"
#include "../components/color.hpp"
#include "../components/lat_lon.hpp"
#include "../components/radius.hpp"
#include "../indicator_component.hpp"
#include "../result.hpp"

#include <cstdint>
#include <optional>
#include <utility>
#include <vector>

namespace rerun::archetypes {
    /// **Archetype**: Geospatial points with positions expressed in [EPSG:4326](https://epsg.io/4326) latitude and longitude (North/East-positive degrees), and optional colors and radii.
    ///
    /// ## Example
    ///
    /// ### Log a geospatial point
    /// ![image](https://static.rerun.io/geopoint_simple/b86ce83e5871837587bd33a0ad639358b96e9010/full.png)
    ///
    /// ```cpp
    /// #include <rerun.hpp>
    ///
    /// int main() {
    ///     const auto rec = rerun::RecordingStream("rerun_example_geo_points");
    ///     rec.spawn().exit_on_failure();
    ///
    ///     rec.log(
    ///         "rerun_hq",
    ///         rerun::GeoPoints::from_lat_lon({{59.319221, 18.075631}})
    ///             .with_radii(rerun::Radius::ui_points(10.0f))
    ///             .with_colors(rerun::Color(255, 0, 0))
    ///     );
    /// }
    /// ```
    struct GeoPoints {
        /// The [EPSG:4326](https://epsg.io/4326) coordinates for the points (North/East-positive degrees).
        Collection<rerun::components::LatLon> positions;

        /// Optional radii for the points, effectively turning them into circles.
        ///
        /// *Note*: scene units radiii are interpreted as meters.
        std::optional<Collection<rerun::components::Radius>> radii;

        /// Optional colors for the points.
        std::optional<Collection<rerun::components::Color>> colors;

        /// Optional class Ids for the points.
        ///
        /// The `components::ClassId` provides colors if not specified explicitly.
        std::optional<Collection<rerun::components::ClassId>> class_ids;

      public:
        static constexpr const char IndicatorComponentName[] =
            "rerun.components.GeoPointsIndicator";

        /// Indicator component, used to identify the archetype when converting to a list of components.
        using IndicatorComponent = rerun::components::IndicatorComponent<IndicatorComponentName>;
        static constexpr const char ArchetypeName[] = "rerun.archetypes.GeoPoints";

      public: // START of extensions from geo_points_ext.cpp:
        /// Creates a new GeoPoints object based on [EPSG:4326](https://epsg.io/4326) latitude and longitude (North/East-positive degrees).
        static GeoPoints from_lat_lon(Collection<components::LatLon> positions_) {
            GeoPoints points;
            points.positions = std::move(positions_);
            return points;
        }

        // END of extensions from geo_points_ext.cpp, start of generated code:

      public:
        GeoPoints() = default;
        GeoPoints(GeoPoints&& other) = default;
        GeoPoints(const GeoPoints& other) = default;
        GeoPoints& operator=(const GeoPoints& other) = default;
        GeoPoints& operator=(GeoPoints&& other) = default;

        explicit GeoPoints(Collection<rerun::components::LatLon> _positions)
            : positions(std::move(_positions)) {}

        /// Optional radii for the points, effectively turning them into circles.
        ///
        /// *Note*: scene units radiii are interpreted as meters.
        GeoPoints with_radii(Collection<rerun::components::Radius> _radii) && {
            radii = std::move(_radii);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional colors for the points.
        GeoPoints with_colors(Collection<rerun::components::Color> _colors) && {
            colors = std::move(_colors);
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        /// Optional class Ids for the points.
        ///
        /// The `components::ClassId` provides colors if not specified explicitly.
        GeoPoints with_class_ids(Collection<rerun::components::ClassId> _class_ids) && {
            class_ids = std::move(_class_ids);
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
    struct AsComponents<archetypes::GeoPoints> {
        /// Serialize all set component batches.
        static Result<std::vector<ComponentBatch>> serialize(const archetypes::GeoPoints& archetype
        );
    };
} // namespace rerun
