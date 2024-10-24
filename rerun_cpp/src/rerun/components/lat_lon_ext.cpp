#include "lat_lon.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct LatLonExt {
            double lat_lon[2];
#define LatLon LatLonExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct LatLon from x/y values.
            LatLon(double lat, double lon) : lat_lon{lat, lon} {}

            double latitude() const {
                return lat_lon.x();
            }

            double longitude() const {
                return lat_lon.y();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
