#include "geo_points.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates a new GeoPoints object based on [EPSG:4326](https://epsg.io/4326) latitude and longitude (North/East-positive degrees).
        static GeoPoints from_lat_lon(Collection<components::LatLon> positions_) {
            GeoPoints points;
            points.positions = std::move(positions_);
            return points;
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif
    } // namespace archetypes
} // namespace rerun
