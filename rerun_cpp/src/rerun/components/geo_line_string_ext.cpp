#include "geo_line_string.hpp"

// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        // <CODEGEN_COPY_TO_HEADER>

        /// Creates a new GeoLineString object based on [EPSG:4326](https://epsg.io/4326) latitude and longitude (North/East-positive degrees).
        static GeoLineString from_lat_lon(Collection<datatypes::DVec2D> lat_lon_) {
            GeoLineString line_string;
            line_string.lat_lon = std::move(lat_lon_);
            return line_string;
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif
    } // namespace components
} // namespace rerun
