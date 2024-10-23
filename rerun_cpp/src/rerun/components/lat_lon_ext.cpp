#include "lat_lon.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct LatLonExt {
            float lat_lon[2];
#define LatLon LatLonExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct LatLon from x/y values.
            LatLon(float lat, float lon) : lat_lon{lat, lon} {}

            float latitude() const {
                return lat_lon.x();
            }

            float longitude() const {
                return lat_lon.y();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
