#include "lat_lon.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct LatLonExt {
            float xy[2];
#define LatLon LatLonExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct LatLon from x/y values.
            LatLon(float x, float y) : xy{x, y} {}

            float x() const {
                return xy.x();
            }

            float y() const {
                return xy.y();
            }

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif
    } // namespace components
} // namespace rerun
