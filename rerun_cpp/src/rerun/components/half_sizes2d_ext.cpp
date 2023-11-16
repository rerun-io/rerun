#include "half_sizes2d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct HalfSizes2DExt {
            float xy[2];
#define HalfSizes2D HalfSizes2DExt

            // <CODEGEN_COPY_TO_HEADER>

            /// Construct HalfSizes2D from x/y values.
            HalfSizes2D(float x, float y) : xy{x, y} {}

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
