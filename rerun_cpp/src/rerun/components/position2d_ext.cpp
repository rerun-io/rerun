#include "position2d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace components {

#ifdef EDIT_EXTENSION
        struct Position2DExt {
            float xy[2];
#define Position2D Position2DExt

            // [CODEGEN COPY TO HEADER START]

            /// Construct Position2D from x/y values.
            Position2D(float x, float y) : xy{x, y} {}

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
