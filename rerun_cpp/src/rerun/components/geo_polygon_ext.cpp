#include "geo_polygon.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace components {
#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates a polygon from latitude/longitude rings, exterior first, then holes.
        static GeoPolygon from_lat_lon(const std::vector<std::vector<encodings::DVec2D>>& rings) {
            std::vector<encodings::DVec2D> positions;
            std::vector<uint64_t> lengths;
            for (const auto& ring : rings) {
                positions.insert(positions.end(), ring.begin(), ring.end());
                lengths.push_back(static_cast<uint64_t>(ring.size()));
            }
            GeoPolygon polygon;
            polygon.polygon.lat_lon = std::move(positions);
            polygon.polygon.ring_lengths = std::move(lengths);
            return polygon;
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif
    } // namespace components
} // namespace rerun
