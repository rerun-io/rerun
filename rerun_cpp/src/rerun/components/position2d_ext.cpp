#include "position2d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct Point2DExt {
            float xy[2];
#define Point2D Point2DExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct Point2D from x/y values.
            Point2D(float x, float y) : xy{x, y} {}

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
