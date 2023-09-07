#include "half_extent2d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct HalfExtent2DExt {
            float xy[2];
#define HalfExtent2D HalfExtent2DExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct HalfExtent2D from x/y values.
            HalfExtent2D(float x, float y) : xy{x, y} {}

            float x() const {
                return xy.x();
            }

            float y() const {
                return xy.y();
            }

            // [CODEGEN COPY TO HEADER END]
        };
#endif
    } // namespace components
} // namespace rerun
